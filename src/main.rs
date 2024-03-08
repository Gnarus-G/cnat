use std::path::PathBuf;

use clap::Parser;
use collect::CollectedClassNames;

#[derive(Parser)]
struct Cli {
    css_file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let c = CollectedClassNames::parse(cli.css_file)?;

    c.report();

    Ok(())
}

mod collect {
    use std::path::PathBuf;

    use swc_core::ecma::utils::swc_common::SourceFile;
    use swc_css::visit::{Visit, VisitWith};

    use swc_css::{ast::Rule, parser::parse_file};

    pub struct CollectedClassNames {
        class_names: Vec<String>,
    }

    impl CollectedClassNames {
        pub fn new() -> Self {
            CollectedClassNames {
                class_names: vec![],
            }
        }

        pub fn report(&self) {
            println!("{:#?}", self.class_names)
        }

        pub fn parse(css_file: PathBuf) -> anyhow::Result<Self> {
            let code = std::fs::read_to_string(&css_file)?;

            let options = swc_css::parser::parser::ParserConfig::default();

            let filename = swc_core::ecma::utils::swc_common::FileName::Real(css_file);

            let cssfile = SourceFile::new_from(
                filename.clone(),
                false,
                filename,
                code.into(),
                swc_core::ecma::utils::swc_common::BytePos(1),
            );

            let mut errors = vec![];
            let c = parse_file::<Vec<Rule>>(&cssfile, None, options, &mut errors).unwrap();

            let mut ccns = CollectedClassNames::new();
            c.into_iter()
                .flat_map(|rule| match rule {
                    Rule::QualifiedRule(rule) => vec![rule],
                    Rule::AtRule(rule) => {
                        let rules = rule.block.expect("the only @ rules should be @media").value;

                        rules
                            .into_iter()
                            .map(|rule| match rule {
                                swc_css::ast::ComponentValue::QualifiedRule(rule) => rule,
                                _ => unreachable!("this type of rule in @media is unsupported"),
                            })
                            .collect::<Vec<_>>()
                    }
                    Rule::ListOfComponentValues(_) => {
                        unreachable!("I don't know what this rule is, but it shouldn't happen.")
                    }
                })
                .flat_map(|rule| match rule.prelude {
                    swc_css::ast::QualifiedRulePrelude::SelectorList(selectors) => {
                        selectors.children
                    }
                    _ => unreachable!("unsupported rule prelude"),
                })
                .for_each(|selectors| {
                    selectors.visit_with(&mut ccns);
                });

            Ok(ccns)
        }
    }

    impl Visit for CollectedClassNames {
        fn visit_complex_selector(&mut self, n: &swc_css::ast::ComplexSelector) {
            n.visit_children_with(self);
        }

        fn visit_complex_selector_children(&mut self, n: &swc_css::ast::ComplexSelectorChildren) {
            match n {
                swc_css::ast::ComplexSelectorChildren::CompoundSelector(selector) => {
                    let selectors = &selector.subclass_selectors;

                    selectors
                        .iter()
                        .filter_map(|s| match s {
                            swc_css::ast::SubclassSelector::Class(selector) => Some(selector),
                            _ => None,
                        })
                        .for_each(|s| {
                            if s.text.value.contains(':') {
                                let cn = s.text.value.split(':').last().unwrap();
                                self.class_names.push(cn.to_string());
                            } else {
                                self.class_names.push(s.text.value.to_string());
                            }
                        });
                }
                swc_css::ast::ComplexSelectorChildren::Combinator(_) => {}
            }
        }
    }
}

mod transform {}
