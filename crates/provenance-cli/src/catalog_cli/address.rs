use provenance_store::operations::catalog::{self, Definition, HttpMethod};
use std::{collections::BTreeMap, sync::OnceLock};

pub(super) struct Resolved {
    pub address: &'static Address,
    pub path: String,
    pub query: Option<&'static str>,
}

#[derive(Clone)]
pub(super) enum Segment {
    Literal(&'static str),
    Parameter(&'static str),
}

pub(super) struct Address {
    pub(super) collection: &'static str,
    pub(super) words: Vec<Segment>,
    pub(super) definition: &'static Definition,
    pub(super) query: Option<&'static str>,
}

pub(super) fn resolve(collection: &str, supplied: &[String]) -> anyhow::Result<Resolved> {
    let candidates = registrations(collection)
        .into_iter()
        .filter(|address| address.words.len() == supplied.len())
        .collect::<Vec<_>>();
    let address = select_address(candidates, supplied).ok_or_else(|| {
        anyhow::anyhow!(
            "the catalog does not declare the {collection} command: {}",
            supplied.join(" ")
        )
    })?;
    let values = address_values(address, supplied).expect("selected address matches");
    Ok(Resolved {
        address,
        path: render_path(address.definition.path, &values)?,
        query: address.query,
    })
}

pub(super) fn help(collection: &str) -> String {
    registrations(collection)
        .into_iter()
        .map(|address| {
            let words = address.words.iter().map(|segment| match segment {
                Segment::Literal(word) => (*word).to_owned(),
                Segment::Parameter(name) => format!("<{name}>"),
            }).collect::<Vec<_>>().join(" ");
            format!("  {collection} {words}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn addresses() -> &'static [Address] {
    static ADDRESSES: OnceLock<Vec<Address>> = OnceLock::new();
    ADDRESSES.get_or_init(build)
}

pub(super) fn registrations(collection: &str) -> Vec<&'static Address> {
    addresses()
        .iter()
        .filter(|address| address.collection == collection)
        .collect()
}

fn build() -> Vec<Address> {
    let definitions = catalog::definitions();
    let mut addresses = Vec::new();
    for definition in definitions {
        let collection = definition.path.split('/').nth(1).expect("route collection");
        let route = route_segments(definition.path);
        let same_path_read = definitions
            .iter()
            .any(|other| other.path == definition.path && other.method == HttpMethod::Get);
        match definition.method {
            HttpMethod::Get if route.is_empty() => {
                push(
                    &mut addresses,
                    collection,
                    vec![literal("list")],
                    definition,
                    None,
                );
            }
            HttpMethod::Post if route.is_empty() => {
                push(
                    &mut addresses,
                    collection,
                    vec![literal("create")],
                    definition,
                    None,
                );
            }
            HttpMethod::Get => {
                let mut suffixed = route.clone();
                suffixed.push(literal("get"));
                push(&mut addresses, collection, suffixed, definition, None);
            }
            HttpMethod::Patch => {
                let mut suffixed = route.clone();
                suffixed.push(literal("update"));
                push(&mut addresses, collection, suffixed, definition, None);
            }
            HttpMethod::Post => {
                let mut form = route.clone();
                if same_path_read {
                    form.push(literal("create"));
                }
                push(&mut addresses, collection, form, definition, None);
            }
        }
        for query in &definition.registration.queries {
            let mut form = route.clone();
            form.push(literal(query.name));
            push(
                &mut addresses,
                collection,
                form,
                definition,
                Some(query.name),
            );
        }
    }
    reject_ambiguous(&addresses);
    addresses
}

fn push(
    addresses: &mut Vec<Address>,
    collection: &'static str,
    words: Vec<Segment>,
    definition: &'static Definition,
    query: Option<&'static str>,
) {
    addresses.push(Address {
        collection,
        words,
        definition,
        query,
    });
}

fn reject_ambiguous(addresses: &[Address]) {
    for (index, address) in addresses.iter().enumerate() {
        for other in &addresses[index + 1..] {
            if address.collection == other.collection
                && address.words.len() == other.words.len()
                && address.words.iter().zip(&other.words).all(|(left, right)| {
                    !matches!((left, right),
                        (Segment::Literal(left), Segment::Literal(right)) if left != right)
                })
            {
                panic!(
                    "ambiguous CLI registrations: {} and {}",
                    address.definition.name, other.definition.name
                );
            }
        }
    }
}

fn route_segments(path: &'static str) -> Vec<Segment> {
    path.split('/')
        .skip(2)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.strip_prefix('{')
                .and_then(|part| part.strip_suffix('}'))
                .map_or(Segment::Literal(part), Segment::Parameter)
        })
        .collect()
}

const fn literal(word: &'static str) -> Segment {
    Segment::Literal(word)
}

fn select_address<'a>(candidates: Vec<&'a Address>, supplied: &[String]) -> Option<&'a Address> {
    let mut matches = candidates
        .into_iter()
        .filter(|address| address_values(address, supplied).is_some());
    let selected = matches.next()?;
    matches.next().is_none().then_some(selected)
}

fn address_values(
    address: &Address,
    supplied: &[String],
) -> Option<BTreeMap<&'static str, String>> {
    let mut values = BTreeMap::new();
    for (expected, actual) in address.words.iter().zip(supplied) {
        match expected {
            Segment::Literal(word) if *word != actual => return None,
            Segment::Literal(_) => {}
            Segment::Parameter(name) if actual.is_empty() => return None,
            Segment::Parameter(name) => {
                values.insert(*name, actual.clone());
            }
        }
    }
    Some(values)
}

fn render_path(pattern: &str, values: &BTreeMap<&'static str, String>) -> anyhow::Result<String> {
    let mut rendered = String::new();
    for part in pattern.split('/').filter(|part| !part.is_empty()) {
        rendered.push('/');
        if let Some(name) = part
            .strip_prefix('{')
            .and_then(|part| part.strip_suffix('}'))
        {
            rendered.push_str(
                values
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("missing registered path parameter {name}"))?,
            );
        } else {
            rendered.push_str(part);
        }
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "ambiguous CLI registrations")]
    fn construction_rejects_intersecting_address_grammars() {
        let definition = &catalog::definitions()[0];
        reject_ambiguous(&[
            Address {
                collection: "sources",
                words: vec![Segment::Parameter("id"), literal("get")],
                definition,
                query: None,
            },
            Address {
                collection: "sources",
                words: vec![literal("get"), Segment::Parameter("id")],
                definition,
                query: None,
            },
        ]);
    }
}
