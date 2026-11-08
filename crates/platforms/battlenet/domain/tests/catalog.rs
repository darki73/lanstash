//! Tests for `lanstash_battlenet_domain::catalog`, on hand-made documents
//! shaped like Battle.net's (no Blizzard files are kept in the repository).

use lanstash_battlenet_domain::catalog::*;
use lanstash_domain::json::parse;

const H1: &str = "ca8e463507cfff95a5759b5550f3eae5";
const H2: &str = "053433e65726aa5f1ae961168e3cb576";

#[test]
fn the_current_catalog_has_the_highest_build_number_not_build_id() {
    let versions = format!(
        "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0|ProductConfig!HEX:16\n\
         ## seqn = 1\n\
         PUB-29|{H2}|x||422|2547|\n\
         PUB-30|{H1}|x||407|5240|\n\
         PUB-31|nothex|x||1|9999|\n"
    );
    assert_eq!(current_build(&versions).as_deref(), Some(H1));
    assert_eq!(current_build(""), None);
}

#[test]
fn base_fragments_skip_overrides_and_encrypted_ones() {
    let root = parse(&format!(
        r#"{{"fragments":[{{"name":"osi","hash":"{H1}"}},{{"name":"osi_cn","hash":"{H2}","requires":["cn"]}},
        {{"name":"rose","hash":"{H2}","decryption_key_id":"k"}},{{"name":"bad","hash":"zz"}}]}}"#
    ))
    .unwrap();
    assert_eq!(base_fragments(&root), vec![("osi".to_owned(), H1.to_owned())]);
}

#[test]
fn a_product_has_codes_a_name_and_art() {
    let frag = parse(&format!(
        r#"{{"installs":{{"osi":{{"tact_product":"osi"}},"osi_test":{{"tact_product":"osit"}}}},
        "products":[{{"id":"OSI","base":{{"name":"osi#OSI_NAME","icon_medium":"osi#OSI_ICON_MEDIUM","key_art":"osi#OSI_KEY_ART"}}}}],
        "strings":{{"default":{{"osi#OSI_NAME":"Diablo II: Resurrected"}}}},
        "files":{{"default":{{"osi#OSI_ICON_MEDIUM":{{"hash":"{H2}"}},"osi#OSI_ICON_MASSIVE":{{"hash":"{H1}"}},"osi#OSI_KEY_ART":{{"hash":"{H2}"}}}}}}}}"#
    ))
    .unwrap();
    assert_eq!(
        product(&frag),
        Some(Product {
            codes: vec!["osi".into(), "osit".into()],
            installs: vec![
                Install { key_art: Some(H2.into()), ..Install::new("osi", Some("Diablo II: Resurrected".into())) },
                Install { key_art: Some(H2.into()), ..Install::new("osit", Some("Diablo II: Resurrected".into())) },
            ],
            name: Some("Diablo II: Resurrected".into()),
            icon: Some(H1.into()),
            key_art: Some(H2.into()),
            background: None,
            logo: None,
        })
    );
    // Without a massive icon, the medium one; without installs, no product.
    let medium = parse(&format!(
        r#"{{"installs":{{"d":{{"tact_product":"drtl"}}}},"products":[{{"base":{{"icon_medium":"d#D_ICON_MEDIUM"}}}}],
        "files":{{"default":{{"d#D_ICON_MEDIUM":{{"hash":"{H2}"}}}}}}}}"#
    ))
    .unwrap();
    assert_eq!(product(&medium).and_then(|p| p.icon).as_deref(), Some(H2));
    assert_eq!(product(&parse(r#"{"products":[{"base":{}}]}"#).unwrap()), None);
}

/// A real fragment in `LANSTASH_FRAGMENT`: how its products and installs are laid out.
#[test]
#[ignore = "needs LANSTASH_FRAGMENT with a real catalog fragment"]
fn a_real_fragment() {
    use lanstash_domain::json::Value;
    let text = std::fs::read_to_string(std::env::var("LANSTASH_FRAGMENT").unwrap()).unwrap();
    let f = parse(&text).unwrap();
    let keys = |v: &Value| v.members().iter().map(|(k, _)| k.clone()).collect::<Vec<_>>().join(",");
    println!("fragment: {}", keys(&f));
    let strings = f.get("strings").and_then(|s| s.get("default"));
    let say = |k: Option<&Value>| {
        k.and_then(Value::as_str)
            .map(|k| strings.and_then(|s| s.get(k)).and_then(Value::as_str).unwrap_or(k).to_owned())
    };
    for (i, p) in f.get("products").map(Value::as_array).unwrap_or_default().iter().enumerate() {
        let base = p.get("base");
        println!("product {i}: keys [{}]; base [{}]", keys(p), base.map(keys).unwrap_or_default());
        println!(
            "  name {:?}; install {:?}",
            say(base.and_then(|b| b.get("name"))),
            p.get("base").and_then(|b| b.get("install_key").or(b.get("install"))).and_then(Value::as_str)
        );
    }
    for (code, inst) in f.get("installs").map(Value::members).unwrap_or_default() {
        println!("install {code}: [{}]", keys(inst));
    }
}

/// A real fragment in `LANSTASH_FRAGMENT`: products, variants and what links them to installs.
#[test]
#[ignore = "needs LANSTASH_FRAGMENT with a real catalog fragment"]
fn a_real_fragment_links() {
    use lanstash_domain::json::Value;
    let text = std::fs::read_to_string(std::env::var("LANSTASH_FRAGMENT").unwrap()).unwrap();
    let f = parse(&text).unwrap();
    let strings = f.get("strings").and_then(|s| s.get("default"));
    let say = |v: Option<&Value>| match v {
        Some(Value::Str(k)) => strings.and_then(|s| s.get(k)).and_then(Value::as_str).unwrap_or(k).to_owned(),
        Some(other) => format!("{other:?}").chars().take(160).collect(),
        None => "-".to_owned(),
    };
    for p in f.get("products").map(Value::as_array).unwrap_or_default() {
        let b = p.get("base");
        let g = |k: &str| say(b.and_then(|b| b.get(k)));
        println!(
            "product id={} name={} real={} override={} type={}",
            say(p.get("id")),
            g("name"),
            g("real_product_id"),
            g("override_product_id"),
            g("default_product_type")
        );
        println!("  play_state_config={}", g("play_state_config"));
        for v in p.get("variants").map(Value::as_array).unwrap_or_default() {
            let keys: Vec<String> = v.members().iter().map(|(k, _)| k.clone()).collect();
            println!("  variant [{}]", keys.join(","));
            for (k, val) in v.members() {
                println!("    {k} = {}", say(Some(val)));
            }
        }
    }
}

/// A real fragment in `LANSTASH_FRAGMENT`: its product types.
#[test]
#[ignore = "needs LANSTASH_FRAGMENT with a real catalog fragment"]
fn a_real_fragment_types() {
    use lanstash_domain::json::Value;
    let text = std::fs::read_to_string(std::env::var("LANSTASH_FRAGMENT").unwrap()).unwrap();
    let f = parse(&text).unwrap();
    let strings = f.get("strings").and_then(|s| s.get("default"));
    let say = |v: &Value| match v {
        Value::Str(k) => strings.and_then(|s| s.get(k)).and_then(Value::as_str).unwrap_or(k).to_owned(),
        other => format!("{other:?}").chars().take(120).collect(),
    };
    let types = f.get("types");
    println!(
        "types is {:?}",
        types.map(|t| match t {
            Value::Array(a) => format!("array of {}", a.len()),
            Value::Object(o) => format!("object of {}", o.len()),
            _ => "other".into(),
        })
    );
    let list: Vec<(String, &Value)> = match types {
        Some(Value::Object(o)) if o.len() == 1 && o[0].0 == "definitions" => {
            o[0].1.as_array().iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect()
        }
        Some(Value::Object(o)) => o.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Some(Value::Array(a)) => a.iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect(),
        _ => Vec::new(),
    };
    for (k, t) in list {
        let fields: Vec<String> = t.members().iter().map(|(n, v)| format!("{n}={}", say(v))).collect();
        if !fields.iter().any(|f| f.contains("cat_in_development") || f.contains("cat_vendor")) {
            println!("type {k}: {}", fields.join(" | "));
        }
    }
}

/// A real fragment in `LANSTASH_FRAGMENT`: each product's variants, raw.
#[test]
#[ignore = "needs LANSTASH_FRAGMENT with a real catalog fragment"]
fn a_real_fragment_variants() {
    use lanstash_domain::json::Value;
    let text = std::fs::read_to_string(std::env::var("LANSTASH_FRAGMENT").unwrap()).unwrap();
    let f = parse(&text).unwrap();
    for p in f.get("products").map(Value::as_array).unwrap_or_default() {
        if let Some(v) = p.get("variants") {
            let s = format!("{v:?}");
            println!("{}: {}", p.get("id").and_then(Value::as_str).unwrap_or("?"), &s[..s.len().min(3000)]);
        }
    }
    let strings = f.get("strings").and_then(|s| s.get("default"));
    for (k, v) in strings.map(Value::members).unwrap_or_default() {
        if k.contains("NAME") || k.contains("TYPES") || k.contains("TITLE") {
            println!("string {k} = {}", v.as_str().unwrap_or("?"));
        }
    }
}

/// A real fragment in `LANSTASH_FRAGMENT` with the real root in `LANSTASH_ROOT`: the product read from them.
#[test]
#[ignore = "needs LANSTASH_FRAGMENT and LANSTASH_ROOT"]
fn a_real_product() {
    let read = |v: &str| parse(&std::fs::read_to_string(std::env::var(v).unwrap()).unwrap()).unwrap();
    let (f, root) = (read("LANSTASH_FRAGMENT"), read("LANSTASH_ROOT"));
    let p = product_with(&f, &global_types(&root)).unwrap();
    println!("name {:?}, icon {:?}, art {:?}", p.name, p.icon, p.key_art);
    for i in &p.installs {
        println!("install {} = {:?}", i.code, i.name);
    }
}

#[test]
fn installs_players_get_and_their_names() {
    // Shaped like World of Warcraft's fragment, much shortened.
    let frag = parse(
        r#"{"installs":{
            "wow":{"tact_product":"wow","auto_update_policy":"x"},
            "wow_classic":{"tact_product":"wow_classic","auto_update_policy":"x"},
            "wow_classic_era":{"tact_product":"wow_classic_era","auto_update_policy":"x"},
            "wowf":{"tact_product":"wowf","auto_update_policy":"x"},
            "wow_ptr":{"tact_product":"wowt","auto_update_policy":"x"},
            "wow_beta":{"tact_product":"wow_beta","auto_update_policy":"x","deprecated":true},
            "wow_dev":{"tact_product":"wowdev"},
            "wow_vendor":{"tact_product":"wowv","auto_update_policy":"x"}},
          "types":{"definitions":[
            {"id":"wow_classic","category":"cat_wow_classic_live","product_defaults":{"type_name":"w#CLASSIC"}},
            {"id":"wow_classic_era","category":"cat_wow_classic_live","product_defaults":{"type_name":"w#CLASSIC"}},
            {"id":"wowf","category":"cat_wow_forever","product_defaults":{"name_style":"default","type_name":"w#FOREVER"}},
            {"id":"wow_vendor","category":"cat_vendor"}]},
          "products":[
            {"id":"WoWC","base":{"name":"w#CLASSIC_NAME","real_product_id":"WoW","default_product_type":"wow_classic"}},
            {"id":"WoW","base":{"name":"w#NAME","default_product_type":"retail"}}],
          "strings":{"default":{"w#NAME":"World of Warcraft","w#CLASSIC_NAME":"World of Warcraft Classic",
            "w#CLASSIC":"Classic","w#FOREVER":"WoW: Forever"}}}"#,
    )
    .unwrap();
    let root = parse(r#"{"types":{"definitions":[{"id":"retail","category":"cat_live"},{"id":"ptr","category":"cat_in_development"}]}}"#).unwrap();
    let p = product_with(&frag, &global_types(&root)).unwrap();
    assert_eq!(p.name.as_deref(), Some("World of Warcraft"), "the main product, not the first");
    let got: Vec<(&str, &str)> =
        p.installs.iter().map(|i| (i.code.as_str(), i.name.as_deref().unwrap_or(""))).collect();
    assert_eq!(
        got,
        vec![
            ("wow", "World of Warcraft"),
            ("wow_classic", "World of Warcraft Classic"),
            ("wow_classic_era", "World of Warcraft Classic"),
            ("wowf", "WoW: Forever"),
        ],
        "ptr (shared in-development type), beta (deprecated), dev (no update policy), vendor: left out"
    );
    assert_eq!(p.codes, vec!["wow", "wow_classic", "wow_classic_era", "wowf"]);
}

/// Real fragments (`*.json`) in `LANSTASH_FRAGMENTS` with the real root in `LANSTASH_ROOT`: each product's installs.
#[test]
#[ignore = "needs LANSTASH_FRAGMENTS and LANSTASH_ROOT"]
fn every_real_product() {
    let dir = std::env::var("LANSTASH_FRAGMENTS").unwrap();
    let root = parse(&std::fs::read_to_string(std::env::var("LANSTASH_ROOT").unwrap()).unwrap()).unwrap();
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    names.sort();
    for path in names {
        let f = parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let codes = product_with(&f, &global_types(&root)).map(|p| p.codes.join(" "));
        println!("{} = {}", path.file_stem().unwrap().to_string_lossy(), codes.unwrap_or_else(|| "-".into()));
    }
}

#[test]
fn each_install_has_the_type_its_product_declares() {
    let root = parse(
        r#"{"types":{"definitions":[{"id":"retail","category":"cat_live"},{"id":"rc","category":"cat_in_development"},
        {"id":"dev","category":"cat_in_development"},{"id":"vendor_1","category":"cat_vendor"}]}}"#,
    )
    .unwrap();
    // Shaped like Hearthstone: its retail install is not the shortest key.
    let hs = parse(
        r#"{"installs":{"hs_beta":{"tact_product":"hsb"},"hs_rc":{"tact_product":"hsrc"}},
        "products":[{"base":{"name":"h#N","types":{"retail":{"uid":"hs_beta"},"rc":{"uid":"hs_rc"}}}}],
        "strings":{"default":{"h#N":"Hearthstone"}}}"#,
    )
    .unwrap();
    assert_eq!(product_with(&hs, &global_types(&root)).unwrap().codes, ["hsb"], "rc left out, hsb is retail");
    // Shaped like Avowed: composite keys whose declared types are dev and rc; a live type of its own kept.
    let aqua = parse(
        r#"{"installs":{"aqua":{"tact_product":"aqua"},"aqua_artbook":{"tact_product":"aqua-artbook"},
        "aqua_retail_dev_1":{"tact_product":"aqua-retail-dev-1"},"aqua_retail_rc":{"tact_product":"aqua-retail-rc"}},
        "types":{"definitions":[{"id":"artbook","category":"cat_live"}]},
        "products":[{"base":{"types":{"retail":{"uid":"aqua"},"artbook":{"uid":"aqua_artbook"},
        "dev":{"uid":"aqua_retail_dev_1"},"rc":{"uid":"aqua_retail_rc"}}}}]}"#,
    )
    .unwrap();
    assert_eq!(product_with(&aqua, &global_types(&root)).unwrap().codes, ["aqua", "aqua-artbook"]);
    // Shaped like Kelp: vendor builds only, its retail type disabled everywhere and without an install.
    let kelp = parse(
        r#"{"installs":{"kelp_vendor_1":{"tact_product":"kelpv1"}},
        "products":[{"base":{"types":{"retail":{"uid":"kelp","region_permission_flags":{"default":"disabled"}},
        "vendor_1":{"uid":"kelp_vendor_1"}}}}]}"#,
    )
    .unwrap();
    assert_eq!(product_with(&kelp, &global_types(&root)), None, "no install players get");
    // Declared in capitals (Diablo's D1H): matched whatever the case.
    let d1 = parse(
        r#"{"installs":{"d1":{"tact_product":"drtl"},"d1h":{"tact_product":"drtl2"}},
        "types":{"definitions":[{"id":"d1_hellfire","category":"cat_live"}]},
        "products":[{"base":{"types":{"retail":{"uid":"D1"},"d1_hellfire":{"uid":"D1H"}}}}]}"#,
    )
    .unwrap();
    assert_eq!(product_with(&d1, &global_types(&root)).unwrap().codes, ["drtl", "drtl2"]);
}

#[test]
fn an_install_is_named_as_the_launchers_version_picker_names_it() {
    // Shaped like World of Warcraft's: three Classic installs the catalog's product names
    // alike, told apart in the launcher's picker.
    let frag = parse(
        r#"{"installs":{"wow":{"tact_product":"wow"},"wow_classic":{"tact_product":"wow_classic"},
        "wow_classic_anniversary":{"tact_product":"wow_anniversary"},"wow_classic_era":{"tact_product":"wow_classic_era"}},
        "types":{"definitions":[{"id":"wow_classic","category":"cat_wow_classic_live","product_defaults":{"type_name":"w#CLASSIC"}},
          {"id":"wow_classic_anniversary","category":"cat_wow_classic_live","product_defaults":{"type_name":"w#CLASSIC"}},
          {"id":"wow_classic_era","category":"cat_wow_classic_live","product_defaults":{"type_name":"w#CLASSIC"}}]},
        "products":[{"base":{"name":"w#NAME","types":{
          "wow_classic":{"uid":"wow_classic","selector_display_name":"w#MOP"},
          "wow_classic_anniversary":{"uid":"wow_classic_anniversary","selector_display_name":"w#BC"},
          "wow_classic_era":{"uid":"wow_classic_era","selector_display_name":"default#UNKNOWN",
            "game_packages":[{"label":"w#SOD","rank":3},{"label":"w#ERA","rank":1}]}}}}],
        "strings":{"default":{"w#NAME":"World of Warcraft","w#CLASSIC":"Classic",
          "w#MOP":"Mists of Pandaria Classic","w#BC":"Burning Crusade Anniversary",
          "w#ERA":"Classic Era","w#SOD":"Season of Discovery"}}}"#,
    )
    .unwrap();
    let p = product_with(&frag, &Types::default()).unwrap();
    let got: Vec<(&str, &str)> =
        p.installs.iter().map(|i| (i.code.as_str(), i.name.as_deref().unwrap_or(""))).collect();
    assert_eq!(
        got,
        [
            ("wow", "World of Warcraft"),
            ("wow_anniversary", "Burning Crusade Anniversary"),
            ("wow_classic", "Mists of Pandaria Classic"),
            ("wow_classic_era", "Classic Era"),
        ],
        "a picker name the fragment does not spell out: the label of its first game mode (by rank)"
    );
}

#[test]
fn an_install_has_its_own_images_else_its_products_else_its_games() {
    const BG: &str = "b0000000000000000000000000000001";
    const CLASSIC_BG: &str = "b0000000000000000000000000000002";
    const CLASSIC_ART: &str = "b0000000000000000000000000000003";
    const LOGO: &str = "b0000000000000000000000000000004";
    // Shaped like World of Warcraft's: a type with its own background, a product of its own
    // (Classic) with its own key art, the game's own images.
    let frag = parse(&format!(
        r#"{{"installs":{{"wow":{{"tact_product":"wow"}},"wow_classic":{{"tact_product":"wow_classic"}}}},
        "products":[
          {{"base":{{"name":"w#C","real_product_id":"WoW","default_product_type":"wow_classic","key_art":"w#C_ART"}}}},
          {{"base":{{"name":"w#N","default_product_type":"retail","background":"w#BG","logo":"w#LOGO",
            "types":{{"wow_classic":{{"uid":"wow_classic","background":"w#C_BG"}}}}}}}}],
        "files":{{"default":{{"w#BG":{{"hash":"{BG}"}},"w#C_BG":{{"hash":"{CLASSIC_BG}"}},
          "w#C_ART":{{"hash":"{CLASSIC_ART}"}},"w#LOGO":{{"hash":"{LOGO}"}}}}}}}}"#
    ))
    .unwrap();
    let p = product(&frag).unwrap();
    assert_eq!((p.background.as_deref(), p.logo.as_deref()), (Some(BG), Some(LOGO)));
    let art = |code: &str| {
        let i = p.installs.iter().find(|i| i.code == code).unwrap();
        (i.background.as_deref(), i.logo.as_deref(), i.key_art.as_deref())
    };
    assert_eq!(art("wow"), (Some(BG), Some(LOGO), None), "the game's");
    assert_eq!(
        art("wow_classic"),
        (Some(CLASSIC_BG), Some(LOGO), Some(CLASSIC_ART)),
        "its type's, its product's, the game's"
    );
}
