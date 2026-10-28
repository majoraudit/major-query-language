#[cfg(test)]
mod variables {
    use std::collections::HashMap;

    use libmql::{
        parse, parse_extern_value, parse_with_externals,
        test_parser::{MQLParser, MQLQueryFile, Rule, Selector},
    };
    use pest::Parser;

    #[test]
    fn name_ok() {
        MQLParser::parse(Rule::variable, "$linearAlgebra").unwrap();
    }

    #[test]
    #[should_panic]
    fn name_not_ok() {
        MQLParser::parse(Rule::variable, "$01linearAlgebra").unwrap();
    }

    #[test]
    fn assignment_class_ok() {
        MQLParser::parse(Rule::var_assign, "$linearAlgebra = CLASS(MATH 2250)").unwrap();
    }

    #[test]
    fn assignment_str_ok() {
        MQLParser::parse(Rule::var_assign, "$loremIpsum = \"this is a string\"").unwrap();
    }

    #[test]
    fn assignment_group_ok() {
        MQLParser::parse(
            Rule::var_assign,
            "$linearAlgebra = [CLASS(MATH 2250), CLASS(MATH 2260)]",
        )
        .unwrap();
    }

    #[test]
    fn assignment_ok() {
        MQLParser::parse(Rule::file, include_str!("inputs/minimal_var_assign.mql")).unwrap();
    }

    #[test]
    fn assignment_multi_grammar_ok() {
        MQLParser::parse(
            Rule::file,
            include_str!("inputs/minimal_var_assign_usage.mql"),
        )
        .unwrap();
    }

    fn expect_p42_desc_math2250_math2260(mql: &MQLQueryFile) {
        assert_eq!(mql.requirements().len(), 1);
        let requirement = &mql.requirements()[0];

        assert_eq!(requirement.description(), "you must take linear algebra");
        assert_eq!(requirement.priority(), 42);

        let query = requirement.query();

        let selectors = query.selector();

        assert_eq!(selectors.len(), 2);

        let [Selector::Class(class_1), Selector::Class(class_2)] = selectors else {
            panic!("expected Class, Class; got {selectors:?}");
        };

        assert_eq!(class_1.department_id(), "MATH");
        assert_eq!(class_2.department_id(), "MATH");

        assert_eq!(class_1.course_number(), 2250);
        assert_eq!(class_2.course_number(), 2260);
    }

    #[test]
    fn assignment_multi_ok() {
        let parse_result = parse(&include_str!("inputs/minimal_var_assign_usage.mql")).unwrap();
        let mql = parse_result.parsed_mql_file();
        expect_p42_desc_math2250_math2260(mql);
    }

    #[test]
    fn assignment_depth_ok() {
        let parse_result = parse(&include_str!("inputs/var_assign_depth.mql")).unwrap();
        let mql = parse_result.parsed_mql_file();
        expect_p42_desc_math2250_math2260(mql);
    }

    #[test]
    #[should_panic(
        expected = "extern variable `$LEVEL` was declared but not provided by the caller"
    )]
    fn extern_missing_var() {
        parse(&include_str!("inputs/minimal_extern.mql")).unwrap();
    }

    #[test]
    fn extern_ok() {
        let level = "L4";
        let description = format!("Must take arabic at the {level} level");

        let parse_result = parse_with_externals(
            &include_str!("inputs/minimal_extern.mql"),
            HashMap::from([
                (
                    "$LEVEL".to_owned(),
                    parse_extern_value(&format!("\"{level}\"")).unwrap(),
                ),
                (
                    "$LEVEL_DESC".to_owned(),
                    parse_extern_value(&format!("\"{description}\"")).unwrap(),
                ),
            ]),
        )
        .unwrap();

        let mql = parse_result.parsed_mql_file();

        assert_eq!(mql.requirements().len(), 1);
        let requirement = &mql.requirements()[0];

        assert_eq!(requirement.description(), description);

        let query = requirement.query();

        let selectors = query.selector();

        assert_eq!(selectors.len(), 1);

        let [Selector::DistCode { code, dist }] = selectors else {
            panic!("expected Class, Class; got {selectors:?}");
        };

        assert_eq!(code, "ARBC");
        assert_eq!(dist, level);
    }
}
