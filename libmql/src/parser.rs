use anyhow::{Context, Result};
use pest::iterators::Pair;
use pest_derive::Parser;
use serde::Serialize;
use std::collections::HashMap;

use crate::{
    LEGACY_LANGUAGE_VERSION,
    closest_string::closest_string,
    yale_departments::{closest_department, is_department},
};

#[derive(Parser)]
#[grammar = "./mql.pest"]
pub struct MQLParser;

#[derive(Debug, Clone, Copy, Serialize)]
pub enum Quantity {
    Single(u16),
    Many { from: u16, to: u16 },
}

#[derive(Debug, Clone, Serialize)]
pub struct Class {
    department_id: String,
    course_number: u16,
    lab: bool,
}

impl Class {
    pub fn department_id(&self) -> &str {
        &self.department_id
    }

    pub fn course_number(&self) -> u16 {
        self.course_number
    }

    pub fn lab(&self) -> bool {
        self.lab
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Argument {
    String(String),
    Class(Class),
}

#[derive(Debug, Clone, Serialize)]
pub enum Selector {
    Class(Class),
    Placement(String),
    Tag(String),
    TagCode {
        tag: String,
        code: String,
    },
    Dist(String),
    DistCode {
        dist: String,
        code: String,
    },
    Range {
        from: Class,
        to: Class,
    },
    RangeDist {
        from: Class,
        to: Class,
        dist: String,
    },
    RangeTag {
        from: Class,
        to: Class,
        tag: String,
    },
    Query(MQLQuery),
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum MQLQueryType {
    Select,
    Limit,
}

#[derive(Debug, Clone, Serialize)]
pub struct MQLQuery {
    quantity: Quantity,
    r#type: MQLQueryType,
    selector: Vec<Selector>,
}

impl MQLQuery {
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    pub fn r#type(&self) -> MQLQueryType {
        self.r#type
    }

    pub fn selector(&self) -> &[Selector] {
        &self.selector
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MQLRequirement {
    query: MQLQuery,
    description: String,
    priority: u16,
}

impl MQLRequirement {
    pub fn query(&self) -> &MQLQuery {
        &self.query
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn priority(&self) -> u16 {
        self.priority
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MQLQueryFile {
    version: &'static str,
    requirements: Vec<MQLRequirement>,
}

impl MQLQueryFile {
    pub fn version(&self) -> &'static str {
        self.version
    }

    pub fn requirements(&self) -> &[MQLRequirement] {
        &self.requirements
    }
}

#[derive(Debug, Clone)]
pub enum VarValue {
    SelectorList(Vec<Selector>),
    Selector(Selector),
    Quantity(Quantity),
    String(String),
    Class(Class),
}

impl VarValue {
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            VarValue::SelectorList(_) => "selector list",
            VarValue::Selector(_) => "selector",
            VarValue::Quantity(_) => "quantity",
            VarValue::String(_) => "string",
            VarValue::Class(_) => "class",
        }
    }
}

type Variables = HashMap<String, VarValue>;

pub(crate) fn renamed_rules_impl(rule: &Rule) -> String {
    match *rule {
        Rule::lpar => "(".to_owned(),
        Rule::rpar => ")".to_owned(),
        Rule::semicolon => ";".to_owned(),
        Rule::string => "<STRING> primitive".to_owned(),
        Rule::class => "<CLASS> primitive".to_owned(),
        Rule::quantity => "quantity: (single: u16) or (multiple: u16-u16)".to_owned(),
        Rule::quantity_single => "quantity (u16)".to_owned(),
        Rule::department_id => "department id (4-symbols)".to_owned(),
        Rule::class_id => "class id (4-digit int)".to_owned(),
        Rule::query_argument => "query argument (<CLASS> or <STRING> primitives)".to_owned(),
        other => format!("{other:?}"),
    }
}

macro_rules! bail_with_span {
    ($span:expr, $($msg:tt)+) => {
        return bail_with_span!(noret $span, $($msg)+)
    };
    (noret $span:expr, $($msg:tt)+) => {
        Err(anyhow::anyhow!(pest::error::Error::new_from_span(pest::error::ErrorVariant::<Rule>::CustomError {
            message: format!($($msg)+)
        }, $span).renamed_rules(renamed_rules_impl)
    ))
    }
}

impl MQLParser {
    // quantity = { quantity_many | quantity_single | variable }
    fn parse_quantity(quantity: Pair<Rule>, vars: &Variables, top_level: bool) -> Result<Quantity> {
        assert_eq!(quantity.as_rule(), Rule::quantity);

        let inner = quantity
            .into_inner()
            .next()
            .context("should have { quantity_single | quantity_multiple | variable }")?;

        match inner.as_rule() {
            Rule::quantity_single => {
                let as_ascii = inner.as_str();
                let as_number = as_ascii
                    .parse::<u16>()
                    .context("selection could not fit as u16")?;

                if as_number == 0 {
                    bail_with_span!(inner.as_span(), "cannot select zero")
                }

                Ok(Quantity::Single(as_number))
            }
            Rule::quantity_many => {
                let mut inner_children = inner.into_inner();
                let from_node = inner_children.next().unwrap();
                let from = from_node
                    .as_str()
                    .parse::<u16>()
                    .context("selection could not fit as u16")?;
                let to_node = inner_children.next().unwrap();
                let to = to_node
                    .as_str()
                    .parse::<u16>()
                    .context("selection could not fit as u16")?;

                if from == 0 && top_level {
                    bail_with_span!(
                        from_node.as_span(),
                        "cannot select a range starting from zero on a top-level SELECT"
                    )
                }

                if to < from {
                    bail_with_span!(
                        to_node.as_span(),
                        "for any SELECT n-k statements, k>=n must be true. {to}<{from}"
                    )
                }

                Ok(Quantity::Many { from, to })
            }
            Rule::variable => {
                let name = inner.as_str();
                let span = inner.as_span();
                match vars.get(name) {
                    Some(VarValue::Quantity(q)) => {
                        if top_level
                            && let Quantity::Many { from, .. } = q
                            && *from == 0
                        {
                            bail_with_span!(
                                span,
                                "cannot select a range starting from zero on a top-level SELECT"
                            );
                        }
                        Ok(*q)
                    }
                    Some(v) => bail_with_span!(
                        span,
                        "expected quantity, found {} (`{name}`)",
                        v.type_name()
                    ),
                    None => bail_with_span!(span, "undefined variable `{name}`"),
                }
            }
            rule => unreachable!(
                "should have {{ quantity_many | quantity_single | variable }}, got {rule:?}"
            ),
        }
    }

    fn parse_selector_list(selector_list: Pair<Rule>, vars: &Variables) -> Result<Vec<Selector>> {
        assert_eq!(selector_list.as_rule(), Rule::selector_list);

        let inner = selector_list.into_inner();
        let mut selectors = vec![];

        for selector_single in inner {
            assert_eq!(selector_single.as_rule(), Rule::selector_single);
            selectors.extend(Self::parse_selector_single(selector_single, vars)?);
        }

        Ok(selectors)
    }

    fn parse_string(string: Pair<Rule>) -> String {
        assert_eq!(string.as_rule(), Rule::string);
        let contents_with_quotes = string.as_str();
        contents_with_quotes[1..contents_with_quotes.len() - 1]
            .replace("\"\"", "\"")
            .to_owned()
    }

    fn parse_class_from_pair(class_argument: Pair<Rule>) -> Result<Class> {
        assert_eq!(class_argument.as_rule(), Rule::class_argument);

        let mut inner_children = class_argument.into_inner();
        let department_id_pair = inner_children.next().context("expected department_id")?;

        if department_id_pair.as_rule() == Rule::bad_department_id {
            bail_with_span!(
                department_id_pair.as_span(),
                "departments must be 3-4 uppercase ASCII character symbols"
            );
        }

        let class_id = inner_children.next().context("expected class_id")?;
        let mut class_id_str = class_id.as_str();
        let lab = class_id_str.ends_with('L');
        if lab {
            class_id_str = &class_id_str[..class_id_str.len() - 1];
        }

        let course_number = class_id_str
            .parse::<u16>()
            .context("could not parse class_id into u16")?;
        let department_id = department_id_pair.as_str();

        if !is_department(department_id) {
            let potential_misspelling = closest_department(department_id);
            bail_with_span!(
                department_id_pair.as_span(),
                "this department does not exist in Yale's course catalog (Did you mean: {potential_misspelling})"
            )
        }

        Ok(Class {
            department_id: department_id.to_owned(),
            course_number,
            lab,
        })
    }

    fn parse_function_argument(argument: Pair<Rule>, vars: &Variables) -> Result<Argument> {
        assert_eq!(argument.as_rule(), Rule::query_argument);

        let inner = argument.into_inner().next().unwrap();

        match inner.as_rule() {
            Rule::string => Ok(Argument::String(Self::parse_string(inner))),
            Rule::class_argument => Ok(Argument::Class(Self::parse_class_from_pair(inner)?)),
            Rule::variable => {
                let name = inner.as_str();
                let span = inner.as_span();
                match vars.get(name) {
                    Some(VarValue::String(s)) => Ok(Argument::String(s.clone())),
                    Some(VarValue::Class(c)) => Ok(Argument::Class(c.clone())),
                    Some(v) => bail_with_span!(
                        span,
                        "expected string or class argument, found {} (`{name}`)",
                        v.type_name()
                    ),
                    None => bail_with_span!(span, "undefined variable `{name}`"),
                }
            }
            rule => {
                unreachable!("should have {{ string | class_argument | variable }}, got {rule:?}")
            }
        }
    }

    // XYZ = { query_name ~ lpar ~ (query_argument ~ ("," ~ query_argument)*) ~ rpar }
    fn parse_xyz(xyz: Pair<Rule>, vars: &Variables) -> Result<Selector> {
        assert_eq!(xyz.as_rule(), Rule::XYZ);
        let span = xyz.as_span();

        let mut inner = xyz.into_inner();

        let query_name = inner.next().context("XYZ should have query_name")?;
        let query_name_span = query_name.as_span();
        assert_eq!(query_name.as_rule(), Rule::query_name);

        let query_name_inner = query_name.into_inner().next().unwrap();

        let mut args = vec![];

        for arg in inner {
            args.push(Self::parse_function_argument(arg, vars)?);
        }

        let selector = match query_name_inner.as_rule() {
            Rule::class => {
                let [Argument::Class(class)] = args.as_slice() else {
                    bail_with_span!(
                        span,
                        "CLASS function must take one <CLASS> argument of the form `DEPARTMENT_ID CLASS_ID`"
                    )
                };
                Selector::Class(class.clone())
            }
            Rule::placement => {
                let [Argument::String(description)] = args.as_slice() else {
                    bail_with_span!(
                        span,
                        "PLACEMENT function must take one <STRING> argument of the form `\"abcXYZ123\"`"
                    )
                };
                Selector::Placement(description.to_owned())
            }
            Rule::tag => {
                let [Argument::String(description)] = args.as_slice() else {
                    bail_with_span!(
                        span,
                        "TAG function must take one <STRING> argument of the form `\"abcXYZ123\"`"
                    )
                };
                Selector::Tag(description.to_owned())
            }
            Rule::range => {
                let [Argument::Class(class_start), Argument::Class(class_end)] = args.as_slice()
                else {
                    bail_with_span!(
                        span,
                        "RANGE function must take two <CLASS>, <CLASS> arguments of the form `DEPARTMENT_ID CLASS_ID`"
                    )
                };
                Selector::Range {
                    from: class_start.clone(),
                    to: class_end.clone(),
                }
            }
            Rule::tag_dept => {
                let [Argument::String(tag), Argument::String(code)] = args.as_slice() else {
                    bail_with_span!(
                        span,
                        "TAG_DEPT function must take two <STRING>, <STRING> arguments of the form `\"YC TAG\"`, `\"Yale Major Code\"`"
                    )
                };
                Selector::TagCode {
                    tag: tag.clone(),
                    code: code.clone(),
                }
            }
            Rule::dist => {
                let [Argument::String(dist)] = args.as_slice() else {
                    bail_with_span!(
                        span,
                        "DIST function must take two <STRING> arguments of the form `\"YC dist\"`"
                    )
                };
                Selector::Dist(dist.clone())
            }
            Rule::dist_dept => {
                let [Argument::String(dist), Argument::String(code)] = args.as_slice() else {
                    bail_with_span!(
                        span,
                        "DIST_DEPT function must take two <STRING>, <STRING> arguments of the form `\"YC dist\"`, `\"Yale Major Code\"`"
                    )
                };
                Selector::DistCode {
                    dist: dist.clone(),
                    code: code.clone(),
                }
            }
            Rule::range_dist => {
                let [
                    Argument::Class(from),
                    Argument::Class(to),
                    Argument::String(dist),
                ] = args.as_slice()
                else {
                    bail_with_span!(
                        span,
                        "RANGE_DIST function must take three <CLASS>, <CLASS>, <STRING> arguments"
                    )
                };

                Selector::RangeDist {
                    from: from.clone(),
                    to: to.clone(),
                    dist: dist.clone(),
                }
            }
            Rule::range_tag => {
                let [
                    Argument::Class(from),
                    Argument::Class(to),
                    Argument::String(tag),
                ] = args.as_slice()
                else {
                    bail_with_span!(
                        span,
                        "RANGE_TAG function must take three <CLASS>, <CLASS>, <STRING> arguments"
                    )
                };

                Selector::RangeTag {
                    from: from.clone(),
                    to: to.clone(),
                    tag: tag.clone(),
                }
            }
            Rule::bad_query => {
                let potential_misspelling = closest_string(
                    query_name_inner.as_str(),
                    [
                        "CLASS",
                        "RANGE",
                        "TAG_DEPT",
                        "TAG",
                        "PLACEMENT",
                        "DIST",
                        "DIST_DEPT",
                        "RANGE_DIST",
                        "RANGE_TAG",
                    ],
                )
                .unwrap();

                bail_with_span!(
                    query_name_span,
                    "not a valid query (Did you mean: {potential_misspelling})"
                )
            }
            _ => unreachable!(),
        };

        Ok(selector)
    }

    // selector_single = { statement | XYZ | variable }
    // Returns Vec<Selector> to allow SelectorList variables to expand in-place.
    fn parse_selector_single(
        selector_single: Pair<Rule>,
        vars: &Variables,
    ) -> Result<Vec<Selector>> {
        assert_eq!(selector_single.as_rule(), Rule::selector_single);

        let inner = selector_single
            .into_inner()
            .next()
            .context("should have { statement | XYZ | class_argument | variable }")?;

        match inner.as_rule() {
            Rule::statement => Ok(vec![Selector::Query(
                Self::parse_query(inner, vars, false).context("could not parse nested query")?,
            )]),
            Rule::XYZ => Ok(vec![Self::parse_xyz(inner, vars)?]),
            Rule::class_argument => Ok(vec![Selector::Class(Self::parse_class_from_pair(inner)?)]),
            Rule::variable => {
                let name = inner.as_str();
                let span = inner.as_span();
                match vars.get(name) {
                    Some(VarValue::Selector(s)) => Ok(vec![s.clone()]),
                    Some(VarValue::SelectorList(list)) => Ok(list.clone()),
                    Some(VarValue::Class(c)) => Ok(vec![Selector::Class(c.clone())]),
                    Some(v) => bail_with_span!(
                        span,
                        "expected selector, found {} (`{name}`)",
                        v.type_name()
                    ),
                    None => bail_with_span!(span, "undefined variable `{name}`"),
                }
            }
            rule => unreachable!(
                "should have {{ statement | XYZ | class_argument | variable }}, got {rule:?}"
            ),
        }
    }

    // selector = { selector_list | selector_single }
    fn parse_selector(selector: Pair<Rule>, vars: &Variables) -> Result<Vec<Selector>> {
        assert_eq!(selector.as_rule(), Rule::selector);

        let inner = selector
            .into_inner()
            .next()
            .context("should have { selector_list | selector_single }")?;

        match inner.as_rule() {
            Rule::selector_list => Self::parse_selector_list(inner, vars),
            Rule::selector_single => Self::parse_selector_single(inner, vars),
            _ => unreachable!("should have {{ selector_list | selector_single }}"),
        }
    }

    fn parse_query(query: Pair<Rule>, vars: &Variables, top_level: bool) -> Result<MQLQuery> {
        assert_eq!(query.as_rule(), Rule::statement);

        // { select ~ quantity ~ from ~ selector }

        let mut inner = query.into_inner();

        let select = inner.next().context(
            "should have { select ~ quantity ~ from ~ selector ~ semicolon }, missing select",
        )?;

        let r#type = match select.as_rule() {
            Rule::select => MQLQueryType::Select,
            Rule::limit if top_level => MQLQueryType::Limit,
            Rule::limit => bail_with_span!(select.as_span(), "LIMIT must be a top-level query"),
            bad => unreachable!("{bad:?}"),
        };

        let quantity = inner.next().context(
            "should have { select ~ quantity ~ from ~ selector ~ semicolon }, missing quantity",
        )?;
        assert_eq!(quantity.as_rule(), Rule::quantity);

        let from = inner.next().context(
            "should have { select ~ quantity ~ from ~ selector ~ semicolon }, missing from",
        )?;
        assert_eq!(from.as_rule(), Rule::from);

        let selector = inner.next().context(
            "should have { select ~ quantity ~ from ~ selector ~ semicolon }, missing selector",
        )?;
        assert_eq!(selector.as_rule(), Rule::selector);

        let quantity =
            Self::parse_quantity(quantity, vars, top_level).context("failed parsing quantity")?;

        let selector = Self::parse_selector(selector, vars).context("failed parsing selector")?;

        Ok(MQLQuery {
            quantity,
            r#type,
            selector,
        })
    }

    // var_value = { selector_list | XYZ | quantity_many | quantity_single | string | class_argument }
    pub(crate) fn parse_var_value(var_value: Pair<Rule>, vars: &Variables) -> Result<VarValue> {
        assert_eq!(var_value.as_rule(), Rule::var_value);

        let inner = var_value
            .into_inner()
            .next()
            .context("expected value in var_value")?;

        match inner.as_rule() {
            Rule::selector_list => Ok(VarValue::SelectorList(Self::parse_selector_list(
                inner, vars,
            )?)),
            Rule::XYZ => Ok(VarValue::Selector(Self::parse_xyz(inner, vars)?)),
            Rule::quantity_many => {
                let mut children = inner.into_inner();
                let from_node = children.next().unwrap();
                let from = from_node
                    .as_str()
                    .parse::<u16>()
                    .context("quantity could not fit as u16")?;
                let to_node = children.next().unwrap();
                let to = to_node
                    .as_str()
                    .parse::<u16>()
                    .context("quantity could not fit as u16")?;
                if to < from {
                    bail_with_span!(
                        to_node.as_span(),
                        "for any range n-k, k>=n must be true. {to}<{from}"
                    );
                }
                Ok(VarValue::Quantity(Quantity::Many { from, to }))
            }
            Rule::quantity_single => {
                let n = inner
                    .as_str()
                    .parse::<u16>()
                    .context("quantity could not fit as u16")?;
                if n == 0 {
                    bail_with_span!(inner.as_span(), "cannot assign zero as a quantity");
                }
                Ok(VarValue::Quantity(Quantity::Single(n)))
            }
            Rule::string => Ok(VarValue::String(Self::parse_string(inner))),
            Rule::class_argument => Ok(VarValue::Class(Self::parse_class_from_pair(inner)?)),
            rule => unreachable!("unexpected var_value inner: {rule:?}"),
        }
    }

    pub fn parse_file(
        root_pair: Pair<Rule>,
        externals: HashMap<String, VarValue>,
    ) -> Result<MQLQueryFile> {
        assert_eq!(root_pair.as_rule(), Rule::file);

        let mut requirements = vec![];
        let mut vars: Variables = HashMap::new();

        let children = root_pair.into_inner();

        for child in children {
            match child.as_rule() {
                Rule::EOI => (),
                Rule::extern_decl => {
                    // extern_decl = { extern_keyword ~ variable }
                    let var_pair = child
                        .into_inner()
                        .next()
                        .context("expected variable in extern_decl")?;
                    assert_eq!(var_pair.as_rule(), Rule::variable);
                    let var_name = var_pair.as_str().to_owned();
                    let span = var_pair.as_span();

                    match externals.get(&var_name).cloned() {
                        Some(value) => {
                            vars.insert(var_name, value);
                        }
                        None => bail_with_span!(
                            span,
                            "extern variable `{var_name}` was declared but not provided by the caller"
                        ),
                    }
                }
                Rule::var_assign => {
                    // var_assign = { variable ~ "=" ~ var_value }
                    let mut inner = child.into_inner();
                    let var_pair = inner.next().context("expected variable in var_assign")?;
                    assert_eq!(var_pair.as_rule(), Rule::variable);
                    let var_name = var_pair.as_str().to_owned();

                    let var_value_pair =
                        inner.next().context("expected var_value in var_assign")?;
                    let value = Self::parse_var_value(var_value_pair, &vars)?;

                    vars.insert(var_name, value);
                }
                Rule::special_statement => {
                    let mut inner = child.into_inner();

                    let statement = inner.next().unwrap();
                    let desc_pair = inner.next().unwrap();

                    let priority = if let Some(priority_pair) = inner.next() {
                        match priority_pair.as_rule() {
                            Rule::quantity_single => {
                                let as_ascii = priority_pair.as_str();
                                as_ascii
                                    .parse::<u16>()
                                    .context("selection could not fit as u16")?
                            }
                            Rule::variable => {
                                let name = priority_pair.as_str();
                                let span = priority_pair.as_span();
                                match vars.get(name) {
                                    Some(VarValue::Quantity(Quantity::Single(n))) => *n,
                                    Some(VarValue::Quantity(_)) => bail_with_span!(
                                        span,
                                        "priority must be a single number, not a range (`{name}`)"
                                    ),
                                    Some(v) => bail_with_span!(
                                        span,
                                        "expected quantity for priority, found {} (`{name}`)",
                                        v.type_name()
                                    ),
                                    None => bail_with_span!(span, "undefined variable `{name}`"),
                                }
                            }
                            rule => unreachable!("unexpected priority rule: {rule:?}"),
                        }
                    } else {
                        1
                    };

                    let description = match desc_pair.as_rule() {
                        Rule::string => Self::parse_string(desc_pair),
                        Rule::variable => {
                            let name = desc_pair.as_str();
                            let span = desc_pair.as_span();
                            match vars.get(name) {
                                Some(VarValue::String(s)) => s.clone(),
                                Some(v) => bail_with_span!(
                                    span,
                                    "expected string for description, found {} (`{name}`)",
                                    v.type_name()
                                ),
                                None => bail_with_span!(span, "undefined variable `{name}`"),
                            }
                        }
                        rule => unreachable!("unexpected description rule: {rule:?}"),
                    };

                    let query = Self::parse_query(statement, &vars, true)?;
                    requirements.push(MQLRequirement {
                        query,
                        description,
                        priority,
                    });
                }
                rule => unreachable!(
                    "should have {{ SOI ~ extern_decl* ~ (var_assign | special_statement)* ~ EOI }}, got {rule:?}"
                ),
            }
        }

        Ok(MQLQueryFile {
            requirements,
            version: LEGACY_LANGUAGE_VERSION,
        })
    }
}
