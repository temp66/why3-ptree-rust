#![allow(non_snake_case)]

use why3_ptree::{
    decl::PropKind,
    ident,
    loc::Position,
    mlw_printer,
    ptree::{Decl, MlwFile, Pty, Qualid},
    ptree_helpers::{self, State},
};

use bumpalo::Bump;
use ocaml_format::{Doc, FormattingOptions};

#[test]
fn test_M1() {
    let mod_M1: (_, Box<[_]>) = {
        // use int.Int
        let use_int_Int = ptree_helpers::r#use(
            Position::default(),
            Box::new(["int".into(), "Int".into()]),
            false,
        );
        // goal g : 2 + 2 = 4
        let g = {
            let two = ptree_helpers::tconst(Position::default(), 2);
            let four = ptree_helpers::tconst(Position::default(), 4);
            let add_int =
                ptree_helpers::qualid(Box::new(["Int".into(), ident::op_infix("+").into()]));
            let two_plus_two =
                ptree_helpers::tapp(Position::default(), add_int, Box::new([two.clone(), two]));
            let eq_int =
                ptree_helpers::qualid(Box::new(["Int".into(), ident::op_infix("=").into()]));
            let goal_term =
                ptree_helpers::tapp(Position::default(), eq_int, Box::new([four, two_plus_two]));
            Decl::Prop(
                PropKind::Goal,
                ptree_helpers::ident(None, Position::default(), "g".into()),
                goal_term,
            )
        };
        (
            ptree_helpers::ident(None, Position::default(), "M1".into()),
            Box::new([use_int_Int, g]),
        )
    };

    let mlw = MlwFile::Modules(Box::new([mod_M1]));
    let arena = Bump::new();
    let mut doc: Doc = Doc::new();
    mlw_printer::pp_mlw_file(None, &mut doc, &arena, &mlw);
    assert_eq!(
        "\
module M1
  use int.Int
  
  goal g: Int.( = ) 4 (Int.( + ) 2 2)
end",
        format!("{}", doc.display(&FormattingOptions::default())),
    );
}

#[test]
fn test_M6() {
    let eq_symb = ptree_helpers::qualid(Box::new([ident::op_infix("=").into()]));
    let int_type_id = ptree_helpers::qualid(Box::new(["int".into()]));
    let int_type = Pty::Tyapp(int_type_id, Box::new([]));
    let ge_int = ptree_helpers::qualid(Box::new(["Int".into(), ident::op_infix(">=").into()]));
    let array_int_type = Pty::Tyapp(
        ptree_helpers::qualid(Box::new(["Array".into(), "array".into()])),
        Box::new([int_type]),
    );
    let length = ptree_helpers::qualid(Box::new(["Array".into(), "length".into()]));
    let array_get = ptree_helpers::qualid(Box::new(["Array".into(), ident::op_get("").into()]));
    let array_set = ptree_helpers::qualid(Box::new(["Array".into(), ident::op_set("").into()]));

    let mlw_file_I = {
        let mut i = State::new();
        i.begin_module(Position::default(), "M6".into());
        i.r#use(
            Position::default(),
            Box::new(["int".into(), "Int".into()]),
            false,
        );
        i.r#use(
            Position::default(),
            Box::new(["array".into(), "Array".into()]),
            false,
        );
        i.begin_let(
            None,
            None,
            None,
            "f".into(),
            ptree_helpers::one_binder(Position::default(), None, Some(array_int_type), "a".into()),
        );
        let id_a = Qualid(Box::new([ptree_helpers::ident(
            None,
            Position::default(),
            "a".into(),
        )]));
        let pre = ptree_helpers::tapp(
            Position::default(),
            ge_int,
            Box::new([
                ptree_helpers::tapp(
                    Position::default(),
                    length,
                    Box::new([ptree_helpers::tvar(Position::default(), id_a.clone())]),
                ),
                ptree_helpers::tconst(Position::default(), 1),
            ]),
        );
        i.add_pre(pre);
        i.add_writes(
            Box::new([ptree_helpers::tvar(Position::default(), id_a.clone())]) as Box<[_]>,
        );
        let post = ptree_helpers::tapp(
            Position::default(),
            eq_symb,
            Box::new([
                ptree_helpers::tapp(
                    Position::default(),
                    array_get,
                    Box::new([
                        ptree_helpers::tvar(Position::default(), id_a.clone()),
                        ptree_helpers::tconst(Position::default(), 0),
                    ]),
                ),
                ptree_helpers::tconst(Position::default(), 42),
            ]),
        );
        i.add_post(post);
        let body = ptree_helpers::eapp(
            Position::default(),
            array_set,
            Box::new([
                ptree_helpers::evar(Position::default(), id_a),
                ptree_helpers::econst(Position::default(), 0),
                ptree_helpers::econst(Position::default(), 42),
            ]),
        );
        i.add_body(body);
        i.end_module();
        i.get_mlw_file()
    };

    let arena = Bump::new();
    let mut doc: Doc = Doc::new();
    mlw_printer::pp_mlw_file(None, &mut doc, &arena, &mlw_file_I);
    assert_eq!(
        "\
module M6
  use int.Int
  use array.Array
  
  let partial f (a : Array.array int)
    requires { Int.( >= ) (a.Array.length) 1 }
    ensures { (Array.( [] ) a 0) = 42 }
  = Array.( []<- ) a 0 42
end",
        format!("{}", doc.display(&FormattingOptions::default())),
    );
}
