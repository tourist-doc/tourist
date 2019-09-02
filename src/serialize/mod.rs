use crate::types::Tour;
use serde::Deserialize;
use serde_json;

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
    use crate::types::path::RelativePathBuf;
    use crate::types::{Stop, StopReference, Tour};
    use quickcheck::{Arbitrary, Gen, QuickCheck, StdThreadGen, TestResult};

    impl Arbitrary for RelativePathBuf {
        fn arbitrary<G: Gen>(g: &mut G) -> Self {
            let mut path: String = Arbitrary::arbitrary(g);
            path.retain(|x| x != '\\');
            RelativePathBuf::from(path)
        }
    }

    impl Arbitrary for StopReference {
        fn arbitrary<G: Gen>(g: &mut G) -> Self {
            StopReference {
                tour_id: Arbitrary::arbitrary(g),
                stop_id: Arbitrary::arbitrary(g),
            }
        }
    }

    impl Arbitrary for Stop {
        fn arbitrary<G: Gen>(g: &mut G) -> Self {
            Stop {
                id: Arbitrary::arbitrary(g),
                title: Arbitrary::arbitrary(g),
                description: Arbitrary::arbitrary(g),
                path: Arbitrary::arbitrary(g),
                repository: Arbitrary::arbitrary(g),
                line: Arbitrary::arbitrary(g),
                children: Arbitrary::arbitrary(g),
            }
        }
    }

    impl Arbitrary for Tour {
        fn arbitrary<G: Gen>(g: &mut G) -> Self {
            Tour {
                protocol_version: latest::PROTOCOL_VERSION.to_owned(),
                id: Arbitrary::arbitrary(g),
                title: Arbitrary::arbitrary(g),
                description: Arbitrary::arbitrary(g),
                stops: Arbitrary::arbitrary(g),
                repositories: Arbitrary::arbitrary(g),
                generator: Arbitrary::arbitrary(g),
            }
        }
    }

    #[test]
    fn latest_is_correct() {
        assert_eq!(latest::PROTOCOL_VERSION, "1.0");
    }

    fn round_trip_quickcheck(t: Tour) -> TestResult {
        TestResult::from_bool(
            dbg!(parse_tour(&serialize_tour(t.clone()).unwrap()).unwrap()) == dbg!(t),
        )
    }

    #[test]
    fn round_trip() {
        QuickCheck::with_gen(StdThreadGen::new(10))
            .tests(100)
            .quickcheck(round_trip_quickcheck as fn(Tour) -> TestResult)
    }
}
