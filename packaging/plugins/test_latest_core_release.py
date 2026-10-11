"""A provider-only release must never be mistaken for a Core binary release."""

import unittest

from latest_core_release import latest_core_tag


class LatestCoreReleaseTests(unittest.TestCase):
    def test_a_newer_provider_release_does_not_hide_a_released_core(self):
        releases = [
            {"tagName": "plugin/example/v1.0.0", "isDraft": False, "isPrerelease": False},
            {"tagName": "0.4.1", "isDraft": False, "isPrerelease": False},
            {"tagName": "0.5.0", "isDraft": True, "isPrerelease": False},
            {"tagName": "0.4.2", "isDraft": False, "isPrerelease": True},
            {"tagName": "0.4.0", "isDraft": False, "isPrerelease": False},
        ]
        self.assertEqual(latest_core_tag(releases), "0.4.1")

    def test_no_core_release_fails_instead_of_downloads_a_provider(self):
        with self.assertRaisesRegex(ValueError, "no stable Core release"):
            latest_core_tag([{"tagName": "plugin/example/v1.0.0"}])
