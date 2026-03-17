#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "${TEST_ROOT}"' EXIT

FAKE_BIN="${TEST_ROOT}/bin"
mkdir -p "${FAKE_BIN}"
REQUESTS_LOG="${TEST_ROOT}/requests.log"

cat > "${FAKE_BIN}/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

payload=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --data)
      payload="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

printf '%s\n' "$payload" >> "${REQUESTS_LOG}"
query="$(jq -r '.query' <<<"$payload")"

if [[ "$query" == *"query BootstrapState"* ]]; then
  if [[ "${LINEAR_FAKE_STATE_MODE:-default}" == "duplicate-team" ]]; then
    cat <<'JSON'
{"data":{"viewer":{"organization":{"id":"org-1","templates":{"nodes":[]}}},"teams":{"nodes":[{"id":"team-1","key":"resq","name":"ResQ","triageEnabled":false,"cyclesEnabled":false,"cycleDuration":1,"cycleCooldownTime":1,"upcomingCycleCount":2,"labels":{"nodes":[]},"states":{"nodes":[]},"templates":{"nodes":[]}},{"id":"team-2","key":"resq","name":"ResQ Duplicate","triageEnabled":false,"cyclesEnabled":false,"cycleDuration":1,"cycleCooldownTime":1,"upcomingCycleCount":2,"labels":{"nodes":[]},"states":{"nodes":[]},"templates":{"nodes":[]}}]}}}
JSON
    exit 0
  fi
  cat <<'JSON'
{"data":{"viewer":{"organization":{"id":"org-1","templates":{"nodes":[]}}},"teams":{"nodes":[{"id":"team-1","key":"resq","name":"ResQ","triageEnabled":false,"cyclesEnabled":false,"cycleDuration":1,"cycleCooldownTime":1,"upcomingCycleCount":2,"labels":{"nodes":[{"id":"label-1","name":"feature","color":"#999999","description":"Old description","isGroup":false}]},"states":{"nodes":[{"id":"state-1","name":"Backlog","type":"backlog","color":"#6b7280","description":"Accepted work not yet scheduled","position":0}]},"templates":{"nodes":[]}}]}}}
JSON
elif [[ "$query" == *"mutation IssueLabelCreate"* ]]; then
  cat <<'JSON'
{"data":{"issueLabelCreate":{"success":true,"issueLabel":{"id":"new-label","name":"bug"}}}}
JSON
elif [[ "$query" == *"mutation IssueLabelUpdate"* ]]; then
  cat <<'JSON'
{"data":{"issueLabelUpdate":{"success":true,"issueLabel":{"id":"label-1","name":"feature"}}}}
JSON
elif [[ "$query" == *"mutation WorkflowStateCreate"* ]]; then
  cat <<'JSON'
{"data":{"workflowStateCreate":{"success":true,"workflowState":{"id":"new-state","name":"Triage"}}}}
JSON
elif [[ "$query" == *"mutation TeamUpdate"* ]]; then
  cat <<'JSON'
{"data":{"teamUpdate":{"success":true,"team":{"id":"team-1","name":"ResQ"}}}}
JSON
elif [[ "$query" == *"mutation TemplateCreate"* ]]; then
  cat <<'JSON'
{"data":{"templateCreate":{"success":true,"template":{"id":"template-1","name":"Release"}}}}
JSON
elif [[ "$query" == *"mutation TemplateUpdate"* ]]; then
  cat <<'JSON'
{"data":{"templateUpdate":{"success":true,"template":{"id":"template-1","name":"Release"}}}}
JSON
else
  echo "Unexpected query:" >&2
  echo "$query" >&2
  exit 1
fi
EOF

chmod +x "${FAKE_BIN}/curl"

TEST_CONFIG="${TEST_ROOT}/resq.test.json"
cat > "${TEST_CONFIG}" <<'JSON'
{
  "workspace": {
    "templates": [
      {
        "name": "Release",
        "description": "Run release checklist",
        "sortOrder": 0,
        "type": "project",
        "templateData": {
          "notes": "release"
        }
      }
    ]
  },
  "team": {
    "key": "resq",
    "settings": {
      "triageEnabled": true,
      "cyclesEnabled": true,
      "cycleDuration": 2,
      "cycleCooldownTime": 0,
      "upcomingCycleCount": 6
    },
    "labels": [
      {
        "name": "bug",
        "color": "#ef4444",
        "description": "Defects",
        "isGroup": false
      },
      {
        "name": "feature",
        "color": "#3b82f6",
        "description": "New capabilities",
        "isGroup": false
      }
    ],
    "workflowStates": [
      {
        "name": "Backlog",
        "type": "backlog",
        "color": "#6b7280",
        "description": "Accepted work not yet scheduled",
        "position": 0
      },
      {
        "name": "Triage",
        "type": "triage",
        "color": "#f59e0b",
        "description": "New work waiting for review",
        "position": 0
      }
    ],
    "templates": [
      {
        "name": "Bug Report",
        "description": "Capture repro details",
        "sortOrder": 0,
        "type": "issue",
        "templateData": {
          "body": "## Steps to reproduce"
        }
      }
    ]
  }
}
JSON

BAD_TEST_CONFIG="${TEST_ROOT}/resq.invalid.json"
cat > "${BAD_TEST_CONFIG}" <<'JSON'
{
  "team": {
    "key": "resq"
  }
}
JSON

export REQUESTS_LOG
export PATH="${FAKE_BIN}:$PATH"
export LINEAR_API_KEY="linear-test-key"
export LINEAR_API_URL="https://linear.example.test/graphql"

SCRIPT="${PROJECT_ROOT}/scripts/linear-bootstrap.sh"

validate_output="$("${SCRIPT}" validate --config "${TEST_CONFIG}")"
[[ "$validate_output" == *"Config valid"* ]]

if "${SCRIPT}" validate --config "${BAD_TEST_CONFIG}" >"${TEST_ROOT}/bad-validate.out" 2>"${TEST_ROOT}/bad-validate.err"; then
  echo "expected validate to fail on malformed config" >&2
  exit 1
fi
grep -qiE 'missing|invalid|error' "${TEST_ROOT}/bad-validate.err"

plan_output="$("${SCRIPT}" plan --config "${TEST_CONFIG}")"
[[ "$plan_output" == *"label.create [resq] bug"* ]]
[[ "$plan_output" == *"label.update [resq] feature"* ]]
[[ "$plan_output" == *"workflow_state.create [resq] Triage"* ]]
[[ "$plan_output" == *"team.update [resq] team-1"* ]]
[[ "$plan_output" == *"template.create [workspace] Release"* ]]
[[ "$plan_output" == *"template.create [resq] Bug Report"* ]]

if LINEAR_FAKE_STATE_MODE="duplicate-team" "${SCRIPT}" plan --config "${TEST_CONFIG}" >"${TEST_ROOT}/duplicate-team.out" 2>"${TEST_ROOT}/duplicate-team.err"; then
  echo "expected plan to fail on duplicate team keys" >&2
  exit 1
fi
grep -qi 'ambiguous' "${TEST_ROOT}/duplicate-team.err"

: > "${REQUESTS_LOG}"
"${SCRIPT}" apply --config "${TEST_CONFIG}" >/dev/null

grep -q 'mutation IssueLabelCreate' "${REQUESTS_LOG}"
grep -q 'mutation IssueLabelUpdate' "${REQUESTS_LOG}"
grep -q 'mutation WorkflowStateCreate' "${REQUESTS_LOG}"
grep -q 'mutation TeamUpdate' "${REQUESTS_LOG}"
grep -q 'mutation TemplateCreate' "${REQUESTS_LOG}"

echo "linear-bootstrap smoke test passed"
