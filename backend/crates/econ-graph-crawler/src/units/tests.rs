//! Table-driven tests over the units phrases the shipped sources produce (their fixtures,
//! curated lists and adapter code), plus phrases that must stay unrecognised.

use super::*;

fn unit(kind: UnitKind, scale: Scale) -> Unit {
    Unit::new(kind, scale)
}

fn money(currency: Option<&str>, scale: Scale) -> Unit {
    let mut u = unit(UnitKind::Currency, scale);
    u.currency = currency.map(str::to_string);
    u
}

fn constant(currency: Option<&str>, scale: Scale, year: Option<i32>) -> Unit {
    let mut u = unit(UnitKind::ConstantPriceCurrency, scale);
    u.currency = currency.map(str::to_string);
    u.price_reference_year = year;
    u
}

fn index(base: Option<&str>) -> Unit {
    let mut u = unit(UnitKind::Index, Scale::Units);
    u.index_base = base.map(str::to_string);
    u
}

fn percent() -> Option<Unit> {
    Some(unit(UnitKind::Percent, Scale::Units))
}

fn assert_cases(source: &str, cases: &[(&str, Option<Unit>)]) {
    for (text, expected) in cases {
        assert_eq!(&parse(text), expected, "{source}: {text:?}");
    }
}

/// FRED's `units` and `units_short` (tests/fixtures/fred/series_*.json) and its index wording.
#[test]
fn fred_phrases() {
    assert_cases(
        "FRED",
        &[
            ("Billions of Dollars", Some(money(None, Scale::Billions))),
            ("Index", Some(index(None))),
            ("Index 2017=100", Some(index(Some("2017=100")))),
            (
                "Billions of Chained 2017 Dollars",
                Some(constant(None, Scale::Billions, Some(2017))),
            ),
            (
                "Thousands of Persons",
                Some(unit(UnitKind::Count, Scale::Thousands)),
            ),
            ("Percent", percent()),
            (
                "Percentage Points",
                Some(unit(UnitKind::PercentagePoints, Scale::Units)),
            ),
            // A recession indicator: not in the vocabulary.
            ("+1 or 0", None),
            // units_short abbreviations are not in the vocabulary.
            ("Bil. of $", None),
        ],
    );
}

/// BLS: the API catalog's `measure_data_type` (tests/fixtures/bls/*.json) and the units
/// discovery builds from the survey series and code files.
#[test]
fn bls_phrases() {
    assert_cases(
        "BLS",
        &[
            ("Index 1982-1984=100", Some(index(Some("1982-1984=100")))),
            ("Index 1982-84=100", Some(index(Some("1982-1984=100")))),
            ("Index 2000=100", Some(index(Some("2000=100")))),
            (
                "Index December 2003=100",
                Some(index(Some("December 2003=100"))),
            ),
            (
                "All Employees, In Thousands",
                Some(unit(UnitKind::Count, Scale::Thousands)),
            ),
            (
                "ALL EMPLOYEES, THOUSANDS",
                Some(unit(UnitKind::Count, Scale::Thousands)),
            ),
            ("Unemployment rate", percent()),
            // Ambiguous: a percentage or some other rate.
            ("Percent or rate", None),
            // JOLTS rate/level labels: neither says what is measured.
            ("Rate", None),
            ("Level - In Thousands", None),
        ],
    );
}

/// BEA: `CL_UNIT` and `METRIC_NAME` (tests/fixtures/bea/*.json), as given and as stored with a
/// prefix that `UNIT_MULT` applied removed.
#[test]
fn bea_phrases() {
    assert_cases(
        "BEA",
        &[
            ("Current Dollars", Some(money(None, Scale::Units))),
            ("Current dollars", Some(money(None, Scale::Units))),
            (
                "Millions of current dollars",
                Some(money(None, Scale::Millions)),
            ),
            ("Chained Dollars", Some(constant(None, Scale::Units, None))),
            ("Fisher Quantity Index", Some(index(None))),
            ("Fisher Price Index", Some(index(None))),
            (
                "Percent change, annual rate",
                Some(unit(UnitKind::PercentChange, Scale::Units)),
            ),
            ("Percent", percent()),
            // CL_UNIT's placeholder for "see METRIC_NAME".
            ("Level", None),
        ],
    );
}

/// World Bank: every unit in data/wdi_indicators.csv.
#[test]
fn world_bank_phrases() {
    let shares = [
        "% of GDP",
        "% of GNI",
        "% of adults",
        "% of available freshwater",
        "% of children ages 12-23 months",
        "% of children under 5",
        "% of current health expenditure",
        "% of energy use",
        "% of government expenditure",
        "% of internal resources",
        "% of labor force",
        "% of labor force ages 15-24",
        "% of land area",
        "% of manufactured exports",
        "% of merchandise exports",
        "% of merchandise imports",
        "% of output",
        "% of people ages 15+",
        "% of people ages 15-24",
        "% of population",
        "% of population ages 15+",
        "% of population ages 15-49",
        "% of population ages 20-79",
        "% of population ages 25+",
        "% of primary school age",
        "% of relevant age group",
        "% of rural population",
        "% of territorial waters",
        "% of total electricity production",
        "% of total final energy consumption",
        "% of total territorial area",
        "% of urban population",
        "% of youth population",
        "%",
        "% per year",
    ];
    let mut cases: Vec<(&str, Option<Unit>)> = shares.iter().map(|s| (*s, percent())).collect();
    let rate = Some(unit(UnitKind::Rate, Scale::Units));
    cases.extend([
        (
            "% change",
            Some(unit(UnitKind::PercentChange, Scale::Units)),
        ),
        ("current US$", Some(money(Some("USD"), Scale::Units))),
        (
            "constant 2015 US$",
            Some(constant(Some("USD"), Scale::Units, Some(2015))),
        ),
        ("index 2010=100", Some(index(Some("2010=100")))),
        ("index 2014-2016=100", Some(index(Some("2014-2016=100")))),
        ("index 2015=100", Some(index(Some("2015=100")))),
        ("per 1000 live births", rate.clone()),
        ("per 1000 people", rate.clone()),
        ("per 100000 live births", rate.clone()),
        ("per 100000 people", rate.clone()),
        ("per million people", rate),
        ("persons", Some(unit(UnitKind::Count, Scale::Units))),
        ("years", Some(unit(UnitKind::Years, Scale::Units))),
        // Not in the vocabulary: ratios, per-capita and per-area amounts, physical units,
        // international (PPP) dollars, a 0-100 score, and "% gross" (a gross enrolment ratio,
        // which can exceed 100).
        ("% gross", None),
        ("LCU per US$", None),
        ("LCU per international $", None),
        ("MJ per 2021 PPP $ of GDP", None),
        ("Mt CO2e", None),
        ("births per 1000 women ages 15-19", None),
        ("births per woman", None),
        ("cubic meters per capita", None),
        ("current US$ per person", None),
        ("current international $", None),
        ("index 0-100", None),
        ("kWh per capita", None),
        ("kg CO2e per 2021 PPP $ of GDP", None),
        ("kg of oil equivalent per capita", None),
        ("kg per hectare", None),
        ("kg per hectare of arable land", None),
        ("micrograms per cubic meter", None),
        ("people per sq. km of land area", None),
        ("t CO2e per capita", None),
    ]);
    assert_cases("World Bank", &cases);

    // The table covers every unit the curated list uses.
    let listed: std::collections::BTreeSet<&str> = include_str!("../../data/wdi_indicators.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .filter_map(|l| l.split_once(',').map(|(_, unit)| unit))
        .collect();
    let covered: std::collections::BTreeSet<&str> = cases.iter().map(|(t, _)| *t).collect();
    let missing: Vec<_> = listed.difference(&covered).collect();
    assert!(
        missing.is_empty(),
        "add these units to the table: {missing:?}"
    );
}

/// Census BDS and FHFA set their units in code.
#[test]
fn census_and_fhfa_phrases() {
    assert_cases(
        "Census, FHFA",
        &[
            ("Count", Some(unit(UnitKind::Count, Scale::Units))),
            (
                "Index (January 1991 = 100)",
                Some(index(Some("January 1991=100"))),
            ),
            ("Index (1991Q1 = 100)", Some(index(Some("1991Q1=100")))),
            ("Index (1980Q1 = 100)", Some(index(Some("1980Q1=100")))),
        ],
    );
}

/// The static catalogs' units (src/sources/static_catalogs.rs).
#[test]
fn static_catalog_phrases() {
    assert_cases(
        "static catalogs",
        &[
            ("Percent per annum", percent()),
            ("Index (2002=100)", Some(index(Some("2002=100")))),
            (
                "Millions of Canadian dollars",
                Some(money(Some("CAD"), Scale::Millions)),
            ),
            (
                "Trillions of yen",
                Some(money(Some("JPY"), Scale::Trillions)),
            ),
            (
                "Millions of euro",
                Some(money(Some("EUR"), Scale::Millions)),
            ),
            (
                "Millions of national currency",
                Some(money(None, Scale::Millions)),
            ),
            (
                "Millions of US dollars",
                Some(money(Some("USD"), Scale::Millions)),
            ),
            ("Index (2011-12=100)", Some(index(Some("2011-2012=100")))),
            (
                "Billions of Swiss francs",
                Some(money(Some("CHF"), Scale::Billions)),
            ),
            (
                "Index (December 2020=100)",
                Some(index(Some("December 2020=100"))),
            ),
            ("Current US dollars", Some(money(Some("USD"), Scale::Units))),
            ("Persons", Some(unit(UnitKind::Count, Scale::Units))),
            ("Years", Some(unit(UnitKind::Years, Scale::Units))),
            ("Various", None),
        ],
    );
}

/// Bank of Canada Valet: the M2 and M2++ group description, and series `metadata.unit`.
#[test]
fn valet_phrases() {
    assert_cases(
        "Valet",
        &[
            (
                "Month-end, Millions of dollars",
                Some(money(None, Scale::Millions)),
            ),
            ("Percent", percent()),
            // An exchange rate: not in the vocabulary.
            ("Canadian dollars per US dollar", None),
        ],
    );
    let cad = UnitsContext {
        dollars: Some("CAD"),
    };
    assert_eq!(
        parse_with("Month-end, Millions of dollars", &cad),
        Some(money(Some("CAD"), Scale::Millions))
    );
    // The context only names a bare "dollars"; a named currency stays as named.
    assert_eq!(
        parse_with("Millions of US dollars", &cad),
        Some(money(Some("USD"), Scale::Millions))
    );
}

/// Only case and whitespace are normalised.
#[test]
fn normalises_case_and_whitespace_only() {
    for text in [
        "billions of dollars",
        "  BILLIONS   OF\tDOLLARS ",
        "Billions\nof Dollars",
    ] {
        assert_eq!(parse(text), Some(money(None, Scale::Billions)), "{text:?}");
    }
    for text in [
        "Billions of Dollars.",
        "Billions-of-Dollars",
        "Billion of Dollars",
        "Bn of Dollars",
        "Billions Dollars",
    ] {
        assert_eq!(parse(text), None, "{text:?}");
    }
}

/// Phrases that must stay unknown: ambiguous, malformed, out of the vocabulary, or a scale on a
/// kind without one.
#[test]
fn unknown_phrases() {
    for text in [
        "",
        "   ",
        "lin",
        "Rate",
        "Level",
        "Dollars per Hour",
        "Dollars per Barrel",
        "Millions of percent",
        "Thousands of Index 2017=100",
        "Billions of years",
        "Index 2017",
        "Index =100",
        "Index 2017=0",
        "Index 2017=-100",
        "Index 1984-1982=100",
        "Index (2017=100",
        "Index 17=100",
        "Index 2017Q5=100",
        "Index Foo 2017=100",
        "Index Janu 2017=100",
        "Chained 17 Dollars",
        "Constant Dollars",
        "Constant 2015",
        "Millions of",
        "% of",
        "Percent change from two years ago",
        "Thousands of Thousands of Persons",
        "Month-end",
        "Month-end, Rate",
        "Dollars, Month-end",
    ] {
        assert_eq!(parse(text), None, "{text:?}");
    }
}

/// Month abbreviations and leading zeros in the base value have one canonical form.
#[test]
fn index_bases_are_canonical() {
    for (text, base) in [
        ("Index Jan 1995=100", "January 1995=100"),
        ("Index Jan. 1995 = 100", "January 1995=100"),
        ("Index 1999-00=100", "1999-2000=100"),
        ("Index 2015=1", "2015=1"),
        ("Index 2015=0100", "2015=100"),
    ] {
        assert_eq!(parse(text), Some(index(Some(base))), "{text:?}");
    }
}

/// The stored JSON form round-trips and leaves absent fields out.
#[test]
fn unit_json_round_trip() {
    let u = constant(Some("USD"), Scale::Billions, Some(2017));
    let json = serde_json::to_value(&u).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "constant_price_currency",
            "currency": "USD",
            "scale": "billions",
            "price_reference_year": 2017
        })
    );
    assert_eq!(serde_json::from_value::<Unit>(json).unwrap(), u);
    assert_eq!(
        serde_json::to_value(index(None)).unwrap(),
        serde_json::json!({"kind": "index", "scale": "units"})
    );
}

#[test]
fn units_source_round_trips() {
    for s in [
        UnitsSource::Structured,
        UnitsSource::ParsedFromText,
        UnitsSource::Unknown,
    ] {
        assert_eq!(UnitsSource::from_db(s.as_str()), Some(s));
    }
    assert_eq!(UnitsSource::from_db("parsed"), None);
}

fn base_units() -> StructuredUnit {
    StructuredUnit {
        scale: Some(Scale::Units),
        ..StructuredUnit::default()
    }
}

/// BEA's stored text agrees with its scaled values: the structured scale is units.
#[test]
fn check_agrees() {
    let ctx = UnitsContext::default();
    assert_eq!(
        check_text("Current dollars", Some(&base_units()), &ctx),
        TextCheck::Agrees(money(None, Scale::Units))
    );
    // Text that leaves out the currency or index base doesn't contradict a structured one.
    let usd = StructuredUnit {
        kind: Some(UnitKind::Currency),
        currency: Some("USD".into()),
        ..StructuredUnit::default()
    };
    assert!(matches!(
        check_text("Billions of Dollars", Some(&usd), &ctx),
        TextCheck::Agrees(_)
    ));
    let based = StructuredUnit {
        index_base: Some("2017=100".into()),
        ..StructuredUnit::default()
    };
    assert!(matches!(
        check_text("Fisher Quantity Index", Some(&based), &ctx),
        TextCheck::Agrees(_)
    ));
}

#[test]
fn check_unparseable() {
    let ctx = UnitsContext::default();
    assert_eq!(
        check_text("Level", Some(&base_units()), &ctx),
        TextCheck::Unrecognised
    );
    assert_eq!(check_text("Level", None, &ctx), TextCheck::Unrecognised);
    assert_eq!(
        check_text("Percent", None, &ctx),
        TextCheck::Parsed(unit(UnitKind::Percent, Scale::Units))
    );
}

/// A BEA text still in millions next to values scaled to dollars; and every other field.
#[test]
fn check_disagrees() {
    let ctx = UnitsContext::default();
    assert_eq!(
        check_text("Millions of current dollars", Some(&base_units()), &ctx),
        TextCheck::Mismatch(money(None, Scale::Millions), vec!["scale"])
    );
    let structured = StructuredUnit {
        kind: Some(UnitKind::Index),
        currency: Some("USD".into()),
        scale: Some(Scale::Units),
        index_base: Some("2012=100".into()),
    };
    let TextCheck::Mismatch(_, fields) = check_text("Index 2017=100", Some(&structured), &ctx)
    else {
        panic!("expected a mismatch")
    };
    assert_eq!(fields, vec!["index_base"]);
    let TextCheck::Mismatch(_, fields) = check_text("Thousands of euro", Some(&structured), &ctx)
    else {
        panic!("expected a mismatch")
    };
    assert_eq!(fields, vec!["kind", "currency", "scale"]);
}

/// The structured fields win in the stored unit; the text fills the rest.
#[test]
fn stored_unit_prefers_structured_fields() {
    let parsed = money(None, Scale::Millions);
    let structured = StructuredUnit {
        currency: Some("USD".into()),
        scale: Some(Scale::Units),
        ..StructuredUnit::default()
    };
    assert_eq!(
        stored_unit(Some(&parsed), Some(&structured)),
        Some(money(Some("USD"), Scale::Units))
    );
    assert_eq!(stored_unit(Some(&parsed), None), Some(parsed.clone()));
    // Structured units without a kind need the text's.
    assert_eq!(stored_unit(None, Some(&structured)), None);
    let full = StructuredUnit {
        kind: Some(UnitKind::Percent),
        scale: Some(Scale::Units),
        ..StructuredUnit::default()
    };
    assert_eq!(
        stored_unit(None, Some(&full)),
        Some(unit(UnitKind::Percent, Scale::Units))
    );
    // A reference year only stays with the kind it came with.
    let chained = constant(None, Scale::Billions, Some(2017));
    let as_index = StructuredUnit {
        kind: Some(UnitKind::Index),
        ..StructuredUnit::default()
    };
    assert_eq!(
        stored_unit(Some(&chained), Some(&as_index)),
        Some(unit(UnitKind::Index, Scale::Billions))
    );
    assert_eq!(stored_unit(None, None), None);
}

/// check_and_record counts each outcome under the source and returns the stored unit.
#[test]
fn check_and_record_counts_by_source() {
    let m = &*CRAWLER_UNITS_METRICS;
    let label = [SourceId::Census.as_str()];
    let counts = || {
        (
            m.unrecognised_total.with_label_values(&label).get(),
            m.mismatch_total.with_label_values(&label).get(),
            m.agree_total.with_label_values(&label).get(),
        )
    };
    let (unrecognised, mismatch, agree) = counts();
    let structured = base_units();

    let stored = check_and_record(SourceId::Census, "a", Some("Count"), Some(&structured));
    assert_eq!(stored, Some(unit(UnitKind::Count, Scale::Units)));
    assert!(counts().2 > agree);

    let stored = check_and_record(
        SourceId::Census,
        "b",
        Some("Thousands of Persons"),
        Some(&structured),
    );
    assert_eq!(stored, Some(unit(UnitKind::Count, Scale::Units)));
    assert!(counts().1 > mismatch);

    assert_eq!(
        check_and_record(SourceId::Census, "c", Some("Level"), Some(&structured)),
        None
    );
    assert_eq!(
        check_and_record(SourceId::Census, "d", Some("+1 or 0"), None),
        None
    );
    assert!(counts().0 >= unrecognised + 2);

    // No text: nothing to check or count.
    assert_eq!(check_and_record(SourceId::Census, "e", None, None), None);
    assert_eq!(
        check_and_record(SourceId::Census, "e", Some("  "), None),
        None
    );
}
