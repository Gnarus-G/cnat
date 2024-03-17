use std::path::PathBuf;

use swc_common::errors::{ColorConfig, Handler};
use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap};
use swc_css::visit::{Visit, VisitWith};

use swc_css::{ast::Rule, parser::parse_file};

pub struct ClassNamesCollector {
    pub class_names: Vec<cnat::Str>,
}

impl ClassNamesCollector {
    pub fn new() -> Self {
        ClassNamesCollector {
            class_names: vec![],
        }
    }

    pub fn parse(css_file: PathBuf) -> anyhow::Result<Self> {
        let code = std::fs::read_to_string(&css_file)?;

        let options = swc_css::parser::parser::ParserConfig::default();

        let cm: Lrc<SourceMap> = Default::default();
        let filename = FileName::Real(css_file);
        let cssfile = cm.new_source_file(filename.clone(), code);

        let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));

        let mut errors = vec![];
        let c = parse_file::<Vec<Rule>>(&cssfile, None, options, &mut errors).unwrap();

        for e in errors {
            e.to_diagnostics(&handler).emit();
        }

        let mut ccns = ClassNamesCollector::new();

        c.visit_with(&mut ccns);

        Ok(ccns)
    }
}

impl Visit for ClassNamesCollector {
    fn visit_compound_selector(&mut self, n: &swc_css::ast::CompoundSelector) {
        let selectors = &n.subclass_selectors;

        selectors
            .iter()
            .filter_map(|s| match s {
                swc_css::ast::SubclassSelector::Class(selector) => Some(selector),
                _ => None,
            })
            .for_each(|s| {
                if s.text.value.contains(':') {
                    let cn = s.text.value.split(':').last().expect("should have at least one value after split, since empty selectors aren't allowed");
                    self.class_names.push(cn.into());
                } else {
                    self.class_names.push(s.text.value.as_str().into());
                }
            });
    }
}
