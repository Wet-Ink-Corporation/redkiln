//! `knowledge-contract` mints hashed atom ids by SPEC ID-04 without linking Core (a contract
//! crate stays light); this checks it against Core's own ID-04 minting, for the kinds Core's
//! `AtomKind` can spell. Core's kind is one word, so `open-question` is not checked here.

use knowledge_contract::{AtomId, HashLen, Kind};
use redkiln_core::ids::{AtomKind, ContentId, HexLen, IdPrefix};

#[test]
fn a_hashed_atom_id_is_cores_id_04_content_id() -> Result<(), Box<dyn std::error::Error>> {
    let one_word = Kind::ALL.into_iter().filter(|k| !k.as_str().contains('-'));
    for kind in one_word {
        for key in ["wi-0123456789", "wi-1b5df8ca7d", ""] {
            let domain = format!("redkiln.id/knowledge.{}/1", kind.as_str());
            let prefix = IdPrefix::Atom(AtomKind::new(kind.as_str())?);
            for (ours, core) in [
                (HashLen::Ten, HexLen::Ten),
                (HashLen::Twelve, HexLen::Twelve),
                (HashLen::Sixteen, HexLen::Sixteen),
            ] {
                let expected = ContentId::mint(prefix.clone(), &domain, &[key], core)?;

                let id = AtomId::hashed(kind, key, ours);

                assert_eq!(id.to_string(), expected.to_string(), "{kind} {key:?}");
            }
        }
    }
    Ok(())
}
