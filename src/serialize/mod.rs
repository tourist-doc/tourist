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

#[cfg(test)]
mod tests {
    use super::latest;

    #[test]
    fn latest_is_correct() {
        assert_eq!(latest::PROTOCOL_VERSION, "1.0");
    }
}
