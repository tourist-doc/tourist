use crate::types::Tour;
use serde::Deserialize;
use serde_json;

pub mod jsonrpc;
pub mod version1;

pub use version1 as latest;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TfProtocol<'a> {
    protocol_version: &'a str,
}

pub fn parse_tour<'a>(s: &'a str) -> Result<Tour, serde_json::Error> {
    let pv: TfProtocol<'a> = serde_json::from_str(s)?;
    Ok(match pv.protocol_version {
        version1::PROTOCOL_VERSION => serde_json::from_str::<version1::TourFile>(s)?.into(),
        _ => panic!("Unexpected protocol version in tour file."),
    })
}

pub fn serialize_tour(tour: Tour) -> Result<String, serde_json::Error> {
    serde_json::to_string(&latest::TourFile::from(tour))
}

#[cfg(test)]
mod tests {
    use super::{latest, parse_tour, serialize_tour};
    use crate::types::Tour;
    use quickcheck::{QuickCheck, StdThreadGen, TestResult};

    #[test]
    fn latest_is_correct() {
        assert_eq!(latest::PROTOCOL_VERSION, "1.0");
    }

    #[test]
    fn round_trip() {
        fn rt(t: Tour) -> TestResult {
            TestResult::from_bool(
                parse_tour(&serialize_tour(t.clone()).expect("serialize fail"))
                    .expect("parse fail")
                    == t,
            )
        }
        QuickCheck::with_gen(StdThreadGen::new(10))
            .tests(100)
            .quickcheck(rt as fn(Tour) -> TestResult)
    }
}
