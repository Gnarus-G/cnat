use std::str::FromStr;

use anyhow::{anyhow, Context};

type Array<T> = Box<[T]>;
type Str = Box<str>;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ScopeVariant {
    AttrNames,
    RecordEntries,
    FnCall,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum MatchType {
    #[default]
    Is,
    Contains,
    StartsWith,
    EndWith,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ScopeValue(MatchType, Str);

#[derive(Debug, PartialEq, Clone)]
pub struct Scope {
    variant: ScopeVariant,
    values: Array<ScopeValue>,
}

impl Scope {
    pub fn matches(&self, s: &str) -> bool {
        for value in self.values.iter() {
            let matches = match value.0 {
                MatchType::Contains => value.1.contains(s),
                MatchType::Is => value.1.contains(s),
                MatchType::StartsWith => value.1.starts_with(s),
                MatchType::EndWith => value.1.ends_with(s),
            };

            if matches {
                return true;
            }
        }

        false
    }
}

impl FromStr for Scope {
    type Err = anyhow::Error;

    ///grammar -> variant:value,value,*value,...,value
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let group = s.split(':').collect::<Vec<_>>();

        let [variant, values] = group.as_slice() else {
            return Err(anyhow!("incorrect number of parts: {:?}", group))
                .context("should be two parts, a variant and values: <variant>:<...values>");
        };

        let values = values
            .split(',')
            .filter(|v| !v.is_empty())
            .map(|v| {
                let mt = MatchType::default();
                ScopeValue(mt, v.into())
            })
            .collect::<Vec<_>>();

        let values = values.into();

        let variant = match *variant {
            "att" => ScopeVariant::AttrNames,
            "prop" => ScopeVariant::RecordEntries,
            "fn" => ScopeVariant::FnCall,
            _ => return Err(anyhow!("unrecognized variant: {}", variant)),
        };

        Ok(Scope { variant, values })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::scope::{MatchType, Scope, ScopeValue, ScopeVariant};

    #[test]
    fn it_parses() {
        assert_eq!(
            Scope::from_str("att:class,className,iconClassName").unwrap(),
            Scope {
                variant: ScopeVariant::AttrNames,
                values: vec![
                    ScopeValue(MatchType::Is, "class".into()),
                    ScopeValue(MatchType::Is, "className".into()),
                    ScopeValue(MatchType::Is, "iconClassName".into())
                ]
                .into_boxed_slice()
            }
        );

        assert_eq!(
            Scope::from_str("prop:classes,className,").unwrap(),
            Scope {
                variant: ScopeVariant::RecordEntries,
                values: vec![
                    ScopeValue(MatchType::Is, "classes".into(),),
                    ScopeValue(MatchType::Is, "className".into())
                ]
                .into_boxed_slice()
            }
        );

        assert_eq!(
            Scope::from_str("fn:cva").unwrap(),
            Scope {
                variant: ScopeVariant::FnCall,
                values: vec![ScopeValue(MatchType::Is, "cva".into())].into_boxed_slice()
            }
        )
    }
}
