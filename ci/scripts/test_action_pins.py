#!/usr/bin/env python3
"""Exercise pin enforcement against GitHub Actions YAML syntax and bypasses."""

import unittest

import yaml

from check_action_pins import check_document


class ActionPinsTest(unittest.TestCase):
    def test_rejects_tags_branches_short_shas_and_docker_tags(self):
        """Reject mutable refs and incomplete commit identifiers."""
        for reference in ("actions/checkout@v7", "owner/repo@main", "owner/repo@1234567",
                          "docker://alpine:latest", "${{ inputs.action }}", ""):
            with self.subTest(reference=reference):
                document = {"jobs": {"build": {"steps": [
                    {"uses": reference, "with": {"persist-credentials": False}}
                ]}}}
                self.assertEqual(len(check_document(document, "fixture")), 1)

    def test_accepts_commit_digest_and_local_actions(self):
        """Allow immutable remote refs and actions versioned with the repository."""
        for reference in ("owner/repo@" + "a" * 40,
                          "owner/repo/sub/action.yml@" + "b" * 40,
                          "docker://alpine@sha256:" + "c" * 64, "./.github/actions/build"):
            with self.subTest(reference=reference):
                self.assertEqual(check_document({"uses": reference}, "fixture"), [])

    def test_yaml_forms_and_reusable_workflows_are_checked(self):
        """Parse action references across YAML forms without scanning shell text."""
        document = yaml.safe_load('''jobs:
  reusable: {uses: "owner/repo/.github/workflows/test.yml@main"}
  build:
    steps:
      - 'uses': >-
          actions/checkout@v7
        with:
          persist-credentials: false
      - run: |
          # uses: this is shell content, not an action
          echo fine
''')
        errors = check_document(document, "fixture")
        self.assertEqual(len(errors), 2)
        self.assertIn("fixture.jobs.reusable.uses", errors[0])

    def test_missing_or_non_string_reference_fails(self):
        """Fail closed when a uses value is not a nonempty string."""
        for value in (None, 123, [], {}):
            self.assertTrue(check_document({"uses": value}, "fixture"))

    def test_checkout_requires_literal_false(self):
        """Missing, enabled, malformed and computed inputs cannot persist tokens."""
        for inputs in (None, {}, {"persist-credentials": True},
                       {"persist-credentials": "false"}, {"persist-credentials": 0},
                       {"persist-credentials": "${{ inputs.persist }}"}, False, []):
            with self.subTest(inputs=inputs):
                document = {"uses": "actions/checkout@" + "a" * 40, "with": inputs}
                errors = check_document(document, "fixture")
                self.assertEqual(len(errors), 1)
                self.assertIn("fixture.with.persist-credentials", errors[0])

    def test_checkout_policy_covers_versions_case_and_composites(self):
        """Check every checkout, including nested composite-action steps."""
        for reference in ("actions/checkout@" + "a" * 40, "Actions/Checkout@" + "b" * 40):
            with self.subTest(reference=reference):
                document = yaml.safe_load(f'''runs:
  using: composite
  steps:
    - uses: {reference}
      with:
        fetch-depth: 0
    - uses: {reference}
      with:
        persist-credentials: false
''')
                errors = check_document(document, "fixture")
                self.assertEqual(len(errors), 1)
                self.assertIn("fixture.runs.steps[0].with.persist-credentials", errors[0])



if __name__ == "__main__":
    unittest.main()
