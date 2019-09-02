use crate::serialize::latest;
use crate::types::path::RelativePathBuf;
use crate::types::{Stop, StopReference, Tour};
use quickcheck::{Arbitrary, Gen};

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
