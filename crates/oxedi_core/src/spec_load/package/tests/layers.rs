use serde_json::json;

use super::super::*;

fn read(layers: serde_json::Value) -> Result<Layers, serde_json::Error> {
    serde_json::from_value(layers)
}

#[test]
fn layers_keep_layer_order() {
    let layers = read(json!(["envelope", "guide"])).unwrap();
    assert_eq!(layers.lowest(), Layer::Envelope);
    assert_eq!(layers.top(), Layer::Guide);
    assert!(layers.contains(Layer::Guide));
    assert!(!layers.contains(Layer::Dictionary));
    assert_eq!(
        layers.iter().collect::<Vec<_>>(),
        [Layer::Envelope, Layer::Guide]
    );
    assert_eq!(layers.to_string(), "\"envelope\", \"guide\"");
}

#[test]
fn no_layer_is_refused() {
    assert_eq!(
        read(json!([])).unwrap_err().to_string(),
        "\"layers\" lists no layer; list the layers the package holds, in layer order"
    );
}

#[test]
fn a_layer_out_of_order_or_twice_is_refused() {
    let expected = "\"layers\" must list each layer once, in layer order (\"envelope\", \"dictionary\", \"transaction-set\", \"guide\", \"views\", \"patch\"); found ";
    for (written, found) in [
        (json!(["guide", "dictionary"]), "\"guide\", \"dictionary\""),
        (json!(["guide", "guide"]), "\"guide\", \"guide\""),
    ] {
        assert_eq!(
            read(written).unwrap_err().to_string(),
            format!("{expected}{found}")
        );
    }
}
