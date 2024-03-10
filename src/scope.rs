use std::str::FromStr;

use anyhow::{anyhow, Context};
use colored::Colorize;

use crate::{Array, Str};

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
    pub fn matches(&self, s: &str, s_variant: ScopeVariant) -> bool {
        for value in self.values.iter() {
            let matches = match value.0 {
                MatchType::Contains => s.contains(&*value.1),
                MatchType::Is => *value.1 == *s,
                MatchType::StartsWith => s.starts_with(&*value.1),
                MatchType::EndWith => s.ends_with(&*value.1),
            };

            if matches && self.variant == s_variant {
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
                .with_context(|| {
                    return format!("\n{}\n\tvariants are one of {}\n\ta value can be a string starting or ending with an '*'", "should be two parts, a variant and values: <variant>:<...values>".yellow(), "att | prop | fn".green())
                });
        };

        let values = values
            .split(',')
            .filter(|v| !v.is_empty())
            .map(|v| {
                let mut mt = MatchType::default();

                let [identifier] = v.split('*').filter(|v| !v.is_empty()).collect::<Vec<_>>()[..]
                else {
                    return Err(anyhow!("cannot have a wildcard in the middle"));
                };

                if v.starts_with('*') && v.ends_with('*') {
                    mt = MatchType::Contains;
                } else if v.starts_with('*') {
                    mt = MatchType::EndWith;
                } else if v.ends_with('*') {
                    mt = MatchType::StartsWith;
                }

                Ok(ScopeValue(mt, identifier.into()))
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?;

        if values.is_empty() {
            return Err(anyhow!("at least one value must be provided"));
        }

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

    #[test]
    fn it_parses_starting_wildcard() {
        assert_eq!(
            Scope::from_str("att:class,*ClassName").unwrap(),
            Scope {
                variant: ScopeVariant::AttrNames,
                values: vec![
                    ScopeValue(MatchType::Is, "class".into()),
                    ScopeValue(MatchType::EndWith, "ClassName".into()),
                ]
                .into_boxed_slice()
            }
        );

        assert_eq!(
            Scope::from_str("prop:classes,***ClassName,").unwrap(),
            Scope {
                variant: ScopeVariant::RecordEntries,
                values: vec![
                    ScopeValue(MatchType::Is, "classes".into(),),
                    ScopeValue(MatchType::EndWith, "ClassName".into())
                ]
                .into_boxed_slice()
            }
        );
    }

    #[test]
    fn it_parses_ending_wildcard() {
        assert_eq!(
            Scope::from_str("att:class,class**").unwrap(),
            Scope {
                variant: ScopeVariant::AttrNames,
                values: vec![
                    ScopeValue(MatchType::Is, "class".into()),
                    ScopeValue(MatchType::StartsWith, "class".into()),
                ]
                .into_boxed_slice()
            }
        );

        assert_eq!(
            Scope::from_str("prop:class**").unwrap(),
            Scope {
                variant: ScopeVariant::RecordEntries,
                values: vec![ScopeValue(MatchType::StartsWith, "class".into())].into_boxed_slice()
            }
        );
    }

    #[test]
    fn it_rejects_middle_wildcard() {
        Scope::from_str("att:class,class*name").unwrap_err();
        Scope::from_str("prop:class*name").unwrap_err();
    }

    #[test]
    fn it_rejects_empty_values() {
        Scope::from_str("att:").unwrap_err();
        Scope::from_str("prop:,").unwrap_err();
    }

    #[test]
    fn it_matches() {
        let scope = Scope {
            variant: ScopeVariant::RecordEntries,
            values: vec![
                ScopeValue(MatchType::Is, "classes".into()),
                ScopeValue(MatchType::Is, "className".into()),
            ]
            .into_boxed_slice(),
        };

        assert!(!scope.matches("className", ScopeVariant::AttrNames));
        assert!(!scope.matches("class", ScopeVariant::RecordEntries));

        assert!(scope.matches("className", ScopeVariant::RecordEntries));
    }

    #[test]
    fn it_matches_ends() {
        let scope = Scope {
            variant: ScopeVariant::AttrNames,
            values: vec![ScopeValue(MatchType::EndWith, "ClassName".into())].into_boxed_slice(),
        };

        assert!(!scope.matches("className", ScopeVariant::AttrNames));
        assert!(!scope.matches("class", ScopeVariant::RecordEntries));

        assert!(scope.matches("iconClassName", ScopeVariant::AttrNames));
        assert!(scope.matches("bodyClassName", ScopeVariant::AttrNames));
        assert!(scope.matches("buttonClassName", ScopeVariant::AttrNames));
    }

    #[test]
    fn it_matches_starts() {
        let scope = Scope {
            variant: ScopeVariant::AttrNames,
            values: vec![ScopeValue(MatchType::StartsWith, "class".into())].into_boxed_slice(),
        };

        assert!(!scope.matches("class", ScopeVariant::RecordEntries));
        assert!(scope.matches("class", ScopeVariant::AttrNames));
        assert!(scope.matches("className", ScopeVariant::AttrNames));
    }

    #[test]
    fn it_matches_contains() {
        let scope = Scope {
            variant: ScopeVariant::AttrNames,
            values: vec![ScopeValue(MatchType::Contains, "class".into())].into_boxed_slice(),
        };

        assert!(!scope.matches("class", ScopeVariant::RecordEntries));
        assert!(scope.matches("class", ScopeVariant::AttrNames));
        assert!(scope.matches("className", ScopeVariant::AttrNames));
        assert!(scope.matches("firstclassName", ScopeVariant::AttrNames));
        assert!(scope.matches("buttonclassName", ScopeVariant::AttrNames));

        let scope = Scope {
            variant: ScopeVariant::AttrNames,
            values: vec![ScopeValue(MatchType::Contains, "Class".into())].into_boxed_slice(),
        };

        assert!(scope.matches("iconClassName", ScopeVariant::AttrNames));
        assert!(scope.matches("bodyClassName", ScopeVariant::AttrNames));
        assert!(scope.matches("buttonClassName", ScopeVariant::AttrNames));
    }
}
