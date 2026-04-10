# No copyright header — resq copyright handles Python files too.

import os

# FAKE credentials for demo — these are not real.
API_KEY = "sk-proj-FAKE_EXAMPLE_KEY_0000000000000000000000000000"
STRIPE_KEY = "sk_test_FAKE_EXAMPLE_KEY_000000000000000000000"
SENDGRID_KEY = "SG.FAKE_EXAMPLE_KEY_00000000000000000000000000000000000"


def start_server():
    print(f"Starting server with API key: {API_KEY[:8]}...")
    print(f"Database: {os.environ.get('DATABASE_URL', 'not set')}")
