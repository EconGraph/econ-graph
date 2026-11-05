use super::*;
use crate::models::company_fact::CompanyFactSubject;
use std::str::FromStr;

fn date(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
}
fn period(start: &str, end: &str) -> ResearchPeriod {
    ResearchPeriod::Duration {
        start: date(start),
        end: date(end),
    }
}
fn fact(
    concept: &str,
    period: ResearchPeriod,
    value: &str,
    accession: &str,
    filed: &str,
) -> CompanyFact {
    CompanyFact::new(
        CompanyFactSubject {
            cik: "0000320193".into(),
            taxonomy: "us-gaap".into(),
            concept: concept.into(),
            period,
            unit: "USD".into(),
        },
        accession.into(),
        date(filed),
        BigDecimal::from_str(value).unwrap(),
        "10-K".into(),
    )
    .unwrap()
}
fn annual(concept: &str, value: &str) -> CompanyFact {
    fact(
        concept,
        period("2023-01-01", "2023-12-31"),
        value,
        "0000320193-24-000001",
        "2024-02-01",
    )
}

#[test]
fn revisions_use_economic_period_and_preserve_original_asof_and_accession() {
    let original = annual("Revenues", "100.000000000000000000001");
    let mut amended = original.clone();
    amended.accession = "0000320193-24-000002".into();
    amended = CompanyFact::new(
        amended.subject.clone(),
        amended.accession.clone(),
        date("2024-03-01"),
        BigDecimal::from(105),
        "10-K/A".into(),
    )
    .unwrap();
    let facts = vec![amended.clone(), original.clone()];
    assert_eq!(
        select_fact_revisions(&facts, &RevisionSelection::Original).unwrap(),
        vec![original.clone()]
    );
    assert_eq!(
        select_fact_revisions(&facts, &RevisionSelection::Latest).unwrap(),
        vec![amended]
    );
    assert_eq!(
        select_fact_revisions(&facts, &RevisionSelection::AsOf(date("2024-02-28"))).unwrap(),
        vec![original.clone()]
    );
    assert_eq!(
        select_fact_revisions(
            &facts,
            &RevisionSelection::Accession(original.accession.clone())
        )
        .unwrap(),
        vec![original]
    );
}

#[test]
fn contradictory_revision_is_rejected_independent_of_input_order() {
    let original = annual("Revenues", "100");
    let mut conflicting = original.clone();
    conflicting.value = BigDecimal::from(101);
    assert!(select_fact_revisions(
        &[original.clone(), conflicting.clone()],
        &RevisionSelection::Latest
    )
    .is_err());
    assert!(select_fact_revisions(&[conflicting, original], &RevisionSelection::Latest).is_err());
    let original = annual("Revenues", "100");
    let mut other_form = original.clone();
    other_form.form = "10-Q".into();
    assert!(select_fact_revisions(&[original, other_form], &RevisionSelection::Latest).is_err());
}

#[test]
fn fourth_quarter_is_exact_and_retains_both_accessions() {
    let annual = annual("Revenues", "100.000000000000000000001");
    let nine = fact(
        "Revenues",
        period("2023-01-01", "2023-09-30"),
        "70.000000000000000000000",
        "0000320193-23-000010",
        "2023-11-01",
    );
    let q4 = derive_quarter(&annual, &nine, date("2024-02-01")).unwrap();
    assert_eq!(
        q4.value,
        BigDecimal::from_str("30.000000000000000000001").unwrap()
    );
    assert_eq!(q4.period, period("2023-10-01", "2023-12-31"));
    assert_eq!(q4.derivation, Some(DerivationKind::FourthQuarter));
    assert_eq!(q4.provenance.len(), 2);
    assert_ne!(q4.provenance[0].accession, q4.provenance[1].accession);
}

#[test]
fn q4_rejects_period_unit_and_later_restatement_mismatches() {
    let annual = annual("Revenues", "100");
    let mut nine = fact(
        "Revenues",
        period("2023-01-01", "2023-09-30"),
        "70",
        "0000320193-23-000010",
        "2023-11-01",
    );
    assert!(derive_quarter(&annual, &nine, date("2024-01-31")).is_err());
    nine.subject.unit = "EUR".into();
    nine = CompanyFact::new(
        nine.subject.clone(),
        nine.accession.clone(),
        nine.filed,
        nine.value.clone(),
        nine.form.clone(),
    )
    .unwrap();
    assert!(derive_quarter(&annual, &nine, date("2024-02-01"))
        .unwrap_err()
        .0
        .contains("same additive concept"));
    nine.subject.unit = "USD".into();
    nine = CompanyFact::new(
        nine.subject.clone(),
        nine.accession.clone(),
        nine.filed,
        nine.value.clone(),
        nine.form.clone(),
    )
    .unwrap();
    nine.filed = date("2024-03-01");
    assert!(derive_quarter(&annual, &nine, date("2024-04-01")).is_err());
    nine.filed = date("2023-11-01");
    nine.subject.period = period("2023-02-01", "2023-09-30");
    nine = CompanyFact::new(
        nine.subject.clone(),
        nine.accession.clone(),
        nine.filed,
        nine.value.clone(),
        nine.form.clone(),
    )
    .unwrap();
    assert!(derive_quarter(&annual, &nine, date("2024-04-01"))
        .unwrap_err()
        .0
        .contains("share a start"));
}

#[test]
fn per_share_and_weighted_share_values_are_reported_but_never_naively_derived() {
    for concept in [
        "EarningsPerShareBasic",
        "EarningsPerShareDiluted",
        "WeightedAverageNumberOfSharesOutstandingBasic",
        "WeightedAverageNumberOfDilutedSharesOutstanding",
    ] {
        let annual = annual(concept, "10");
        let nine = fact(
            concept,
            period("2023-01-01", "2023-09-30"),
            "7",
            "0000320193-23-000010",
            "2023-11-01",
        );
        assert!(derive_quarter(&annual, &nine, date("2024-03-01")).is_err());
        let lines = normalized_statement(
            &[annual, nine],
            StatementKind::IncomeStatement,
            &RevisionSelection::Latest,
        )
        .unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|line| line.derivation.is_none()));
    }
}

#[test]
fn ttm_sums_only_contiguous_compatible_quarters() {
    let quarters = [
        ("2023-01-01", "2023-03-31", "10"),
        ("2023-04-01", "2023-06-30", "20"),
        ("2023-07-01", "2023-09-30", "30"),
        ("2023-10-01", "2023-12-31", "40"),
    ];
    let facts: Vec<_> = quarters
        .into_iter()
        .enumerate()
        .map(|(i, (start, end, value))| {
            fact(
                "Revenues",
                period(start, end),
                value,
                &format!("0000320193-24-{:06}", i + 1),
                "2024-02-01",
            )
        })
        .collect();
    let mapping = standard_concept("us-gaap", "Revenues", StatementKind::IncomeStatement).unwrap();
    let mut lines: Vec<_> = facts.iter().map(|f| fact_line(f, mapping, false)).collect();
    let ttm = derive_ttm(&lines, date("2024-02-01")).unwrap();
    assert_eq!(ttm.value, BigDecimal::from(100));
    assert_eq!(ttm.provenance.len(), 4);
    lines[2].period = period("2023-07-02", "2023-09-30");
    assert!(derive_ttm(&lines, date("2024-02-01")).is_err());
    lines[2].period = period("2023-07-01", "2023-09-30");
    lines[2].unit = "EUR".into();
    assert!(derive_ttm(&lines, date("2024-02-01")).is_err());
}

#[test]
fn normalized_statements_ignore_misleading_fiscal_labels_and_flag_value_changes() {
    let original = annual("Revenues", "100");
    let mut amended = fact(
        "Revenues",
        original.subject.period.clone(),
        "110",
        "0000320193-25-000001",
        "2025-02-01",
    );
    amended.fiscal_year = Some(2024);
    amended.fiscal_period = Some("FY".into());
    amended.frame = Some("CY2023".into());
    let lines = normalized_statement(
        &[original, amended],
        StatementKind::IncomeStatement,
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert_eq!(lines[0].period, period("2023-01-01", "2023-12-31"));
    assert!(lines[0].restated);
}

#[test]
fn ratios_expose_missing_zero_denominators_and_exact_inputs() {
    let gross = annual("GrossProfit", "25");
    let revenue = annual("Revenues", "100");
    let ratios = calculate_ratios(
        &[gross.clone(), revenue.clone()],
        &gross.subject.period,
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert_eq!(ratios[0].name, "gross_margin");
    assert_eq!(ratios[0].value, Some(BigDecimal::from_str("0.25").unwrap()));
    assert_eq!(ratios[0].provenance.len(), 2);
    assert!(ratios[1].unavailable_reason.is_some());
    let mut zero = revenue;
    zero.value = BigDecimal::from(0);
    let ratios = calculate_ratios(
        &[gross.clone(), zero],
        &gross.subject.period,
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert!(ratios[0].value.is_none());
    assert_eq!(
        ratios[0].unavailable_reason.as_deref(),
        Some("denominator is zero")
    );
}

#[test]
fn ratio_rejects_currency_mismatch_and_ambiguous_revenue_aliases() {
    let gross = annual("GrossProfit", "25");
    let mut revenue = annual("Revenues", "100");
    revenue.subject.unit = "EUR".into();
    revenue = CompanyFact::new(
        revenue.subject.clone(),
        revenue.accession.clone(),
        revenue.filed,
        revenue.value.clone(),
        revenue.form.clone(),
    )
    .unwrap();
    let ratios = calculate_ratios(
        &[gross.clone(), revenue],
        &gross.subject.period,
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert_eq!(
        ratios[0].unavailable_reason.as_deref(),
        Some("inputs must use the same taxonomy and unit")
    );
    let ratios = calculate_ratios(
        &[
            gross.clone(),
            annual("Revenues", "100"),
            annual("SalesRevenueNet", "100"),
        ],
        &gross.subject.period,
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert!(ratios[0].value.is_none());
}

#[test]
fn balance_sheet_uses_instant_facts_without_summing_periods() {
    let p = ResearchPeriod::Instant {
        date: date("2023-12-31"),
    };
    let assets = fact(
        "Assets",
        p.clone(),
        "150",
        "0000320193-24-000001",
        "2024-02-01",
    );
    let liabilities = fact(
        "Liabilities",
        p.clone(),
        "100",
        "0000320193-24-000001",
        "2024-02-01",
    );
    let equity = fact(
        "StockholdersEquity",
        p,
        "50",
        "0000320193-24-000001",
        "2024-02-01",
    );
    let lines = normalized_statement(
        &[assets, liabilities, equity],
        StatementKind::BalanceSheet,
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|line| line.derivation.is_none()));
}

#[test]
fn diagnostics_explain_missing_inputs_and_nonadditive_per_share_values() {
    let diagnostics = statement_derivation_diagnostics(
        &[
            annual("Revenues", "100"),
            annual("EarningsPerShareBasic", "10"),
        ],
        StatementKind::IncomeStatement,
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics
        .iter()
        .any(|d| d.concept == "Revenues" && d.reason.contains("missing or ambiguous")));
    assert!(diagnostics
        .iter()
        .any(|d| d.concept == "EarningsPerShareBasic" && d.reason.contains("cannot be derived")));
}

#[test]
fn ambiguous_cumulative_boundaries_do_not_produce_arbitrary_q4() {
    let facts = vec![
        annual("Revenues", "100"),
        fact(
            "Revenues",
            period("2023-01-01", "2023-09-30"),
            "70",
            "0000320193-23-000010",
            "2023-11-01",
        ),
        fact(
            "Revenues",
            period("2023-01-01", "2023-09-29"),
            "69",
            "0000320193-23-000011",
            "2023-11-01",
        ),
    ];
    let lines = normalized_statement(
        &facts,
        StatementKind::IncomeStatement,
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert!(lines.iter().all(|line| line.derivation.is_none()));
    let diagnostics = statement_derivation_diagnostics(
        &facts,
        StatementKind::IncomeStatement,
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].reason.contains("ambiguous"));
}

#[test]
fn derived_q4_margin_exposes_all_four_source_operands() {
    let facts = vec![
        annual("Revenues", "100"),
        annual("GrossProfit", "40"),
        fact(
            "Revenues",
            period("2023-01-01", "2023-09-30"),
            "60",
            "0000320193-23-000010",
            "2023-11-01",
        ),
        fact(
            "GrossProfit",
            period("2023-01-01", "2023-09-30"),
            "30",
            "0000320193-23-000010",
            "2023-11-01",
        ),
    ];
    let ratios = calculate_ratios(
        &facts,
        &period("2023-10-01", "2023-12-31"),
        &RevisionSelection::Latest,
    )
    .unwrap();
    assert_eq!(ratios[0].value, Some(BigDecimal::from_str("0.25").unwrap()));
    assert_eq!(ratios[0].provenance.len(), 4);
}

#[test]
fn unsupported_concepts_and_period_shapes_remain_visible_as_unclassified_facts() {
    let revenue = annual("Revenues", "100");
    let unknown = annual("SomeStandardConceptOutsideTheMapping", "7");
    let wrong_period = annual("Assets", "200");
    let facts = vec![revenue, unknown.clone(), wrong_period.clone()];
    let unclassified = unclassified_statement_facts(&facts, &RevisionSelection::Latest).unwrap();
    assert_eq!(unclassified.len(), 2);
    assert!(unclassified.contains(&unknown));
    assert!(unclassified.contains(&wrong_period));
}

#[test]
fn apple_and_chevron_statement_lines_retain_source_values_and_distinct_cost_categories() {
    // Source-grounded examples from the repository's apple_2025_q3_10q.xml and
    // chevron_2025_q2_10q.xml whole-entity contexts. These verify normalization;
    // accession/filed metadata is synthetic. These are not companyfacts API captures.
    for (cik,concept,value,kind,expected_key) in [
        ("0000320193","NonoperatingIncomeExpense","-171000000",StatementKind::IncomeStatement,"nonoperating_income"),
        ("0000320193","IncomeLossFromContinuingOperationsBeforeIncomeTaxesExtraordinaryItemsNoncontrollingInterest","28031000000",StatementKind::IncomeStatement,"pretax_income"),
        ("0000320193","AssetsNoncurrent","209004000000",StatementKind::BalanceSheet,"noncurrent_assets"),
        ("0000320193","RetainedEarningsAccumulatedDeficit","-17607000000",StatementKind::BalanceSheet,"retained_earnings"),
        ("0000320193","PaymentsToAcquireAvailableForSaleSecuritiesDebt","17591000000",StatementKind::CashFlow,"debt_security_purchases"),
        ("0000093410","OperatingCostsAndExpenses","6674000000",StatementKind::IncomeStatement,"operating_costs_and_expenses"),
        ("0000093410","CostsAndExpenses","40675000000",StatementKind::IncomeStatement,"total_costs_and_expenses"),
        ("0000093410","IncomeLossFromEquityMethodInvestments","1356000000",StatementKind::IncomeStatement,"equity_method_income"),
        ("0000093410","InterestPaidNet","410000000",StatementKind::CashFlow,"interest_paid_net"),
    ] {
        let p=if kind==StatementKind::BalanceSheet {ResearchPeriod::Instant{date:date("2025-06-28")}} else {period("2025-04-01","2025-06-30")};
        let mut source=fact(concept,p,value,"0000320193-25-000001","2025-08-01");
        source.subject.cik=cik.into();
        source=CompanyFact::new(source.subject.clone(),source.accession.clone(),source.filed,source.value.clone(),source.form.clone()).unwrap();
        let lines=normalized_statement(&[source.clone()],kind,&RevisionSelection::Latest).unwrap();
        assert_eq!(lines.len(),1);assert_eq!(lines[0].key,expected_key);assert_eq!(lines[0].value,source.value);assert_eq!(lines[0].provenance[0].fact_id,source.id);
    }
}
