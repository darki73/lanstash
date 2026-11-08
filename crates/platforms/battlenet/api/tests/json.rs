//! Battle.net's views as JSON: the fields the web app reads.

use lanstash_battlenet_api::json::{build, catalog, detail};
use lanstash_battlenet_app::builds::BuildState;
use lanstash_battlenet_app::products::CatalogProduct;
use lanstash_battlenet_domain::plan::{Coverage, PlanDetail};
use lanstash_battlenet_domain::tact::Version;

fn state(coverage: Option<Coverage>) -> BuildState {
    BuildState {
        version: Version { build_config: "b1".into(), cdn_config: "c1".into(), name: "3.3.7".into() },
        coverage,
        plan_error: None,
    }
}

#[test]
fn a_build_says_how_much_of_it_is_held() {
    assert_eq!(
        build(&state(Some(Coverage { have: 5, want: 5 }))),
        "{\"version\":\"3.3.7\",\"build_config\":\"b1\",\"have\":5,\"want\":5,\"complete\":true,\"plan_error\":null}"
    );
    assert_eq!(
        build(&state(None)),
        "{\"version\":\"3.3.7\",\"build_config\":\"b1\",\"have\":null,\"want\":null,\"complete\":false,\"plan_error\":null}",
        "unplanned"
    );
}

#[test]
fn plan_figures_are_null_until_planned() {
    assert_eq!(detail(None), "null");
    let d = PlanDetail {
        files: Coverage { have: 1, want: 2 },
        archived: Coverage { have: 3, want: 4 },
        file_count: 5,
        part_count: 6,
        archive_count: 7,
        requests: 8,
        absent: 9,
    };
    assert_eq!(
        detail(Some(d)),
        "{\"files_have\":1,\"files_want\":2,\"archived_have\":3,\"archived_want\":4,\"file_count\":5,\
         \"part_count\":6,\"archive_count\":7,\"requests\":8,\"absent\":9}"
    );
}

#[test]
fn the_catalog_lists_codes_names_and_versions() {
    let list = [CatalogProduct {
        code: "osi".into(),
        name: Some("Diablo \"II\"".into()),
        family: Some("Diablo".into()),
        game: Some("osi".into()),
        version: None,
        known: true,
    }];
    assert_eq!(
        catalog(&list),
        "{\"products\":[{\"code\":\"osi\",\"name\":\"Diablo \\\"II\\\"\",\"family\":\"Diablo\",\"game\":\"osi\",\"version\":null,\"known\":true}]}\n"
    );
}
