#!/usr/bin/env bash

# Copyright 2026 ResQ
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# shellcheck source=lib/shell-utils.sh
source "${SCRIPT_DIR}/lib/shell-utils.sh"

CONFIG_PATH="${PROJECT_ROOT}/config/linear/resq.json"
LINEAR_API_URL="${LINEAR_API_URL:-https://api.linear.app/graphql}"

usage() {
    cat <<'EOF'
Usage:
  ./scripts/linear-bootstrap.sh validate [--config path]
  ./scripts/linear-bootstrap.sh plan [--config path]
  ./scripts/linear-bootstrap.sh apply [--config path]

Required environment:
  LINEAR_API_KEY   Personal API key for plan/apply

Optional environment:
  LINEAR_API_URL   Override GraphQL endpoint (used by tests)
EOF
}

die() {
    log_error "$*"
    exit 1
}

require_command() {
    command_exists "$1" || die "Missing required command: $1"
}

json_escape_query() {
    jq -Rs . <<<"$1"
}

linear_graphql() {
    local query="$1"
    local variables_json="${2-}"
    local payload

    if [[ -z "${variables_json}" ]]; then
        variables_json='{}'
    fi

    payload="$(jq -n \
        --arg query "$query" \
        --argjson variables "$variables_json" \
        '{ query: $query, variables: $variables }')"

    curl -sS \
        -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: ${LINEAR_API_KEY}" \
        --data "$payload" \
        "${LINEAR_API_URL}"
}

linear_request() {
    local query="$1"
    local variables_json="${2-}"
    local response

    response="$(linear_graphql "$query" "$variables_json")"

    if jq -e '.errors and (.errors | length > 0)' >/dev/null <<<"$response"; then
        jq -r '.errors[] | .message' <<<"$response" >&2
        exit 1
    fi

    printf '%s\n' "$response"
}

validate_config() {
    jq -e '
      .workspace.templates and
      .team.key and
      .team.settings and
      .team.labels and
      .team.workflowStates and
      .team.templates
    ' "$CONFIG_PATH" >/dev/null || die "Config missing required top-level sections"

    jq -e '
      .team.labels | all(
        .name and
        .color and
        has("isGroup")
      )
    ' "$CONFIG_PATH" >/dev/null || die "Every team label must include name, color, and isGroup"

    jq -e '
      .team.workflowStates | all(
        .name and
        .type and
        (.position | type == "number")
      )
    ' "$CONFIG_PATH" >/dev/null || die "Every workflow state must include name, type, and numeric position"

    jq -e '
      (.workspace.templates + .team.templates) | all(
        .name and
        .type and
        .templateData
      )
    ' "$CONFIG_PATH" >/dev/null || die "Every template must include name, type, and templateData"
}

fetch_state() {
    local query='query BootstrapState {
  viewer {
    organization {
      id
      templates {
        nodes {
          id
          name
          description
          sortOrder
          type
          templateData
          team {
            id
            key
          }
        }
      }
    }
  }
  teams {
    nodes {
      id
      key
      name
      triageEnabled
      cyclesEnabled
      cycleDuration
      cycleCooldownTime
      upcomingCycleCount
      labels {
        nodes {
          id
          name
          color
          description
          isGroup
        }
      }
      states {
        nodes {
          id
          name
          type
          color
          description
          position
        }
      }
      templates {
        nodes {
          id
          name
          description
          sortOrder
          type
          templateData
          team {
            id
            key
          }
        }
      }
    }
  }
}'
    linear_request "$query"
}

append_action() {
    local actions_json="$1"
    local action_json="$2"
    jq -c --argjson action "$action_json" '. + [$action]' <<<"$actions_json"
}

json_equals() {
    local left="$1"
    local right="$2"
    [[ "$(jq -cS . <<<"$left")" == "$(jq -cS . <<<"$right")" ]]
}

require_unique_match() {
    local output_var="$1"
    local matches_json="$2"
    local not_found_message="$3"
    local ambiguous_message="$4"
    local match_count

    match_count="$(jq 'length' <<<"$matches_json")"
    case "$match_count" in
        0) die "$not_found_message" ;;
        1) printf -v "$output_var" '%s' "$(jq -c '.[0]' <<<"$matches_json")" ;;
        *) die "$ambiguous_message" ;;
    esac
}

optional_unique_match() {
    local output_var="$1"
    local matches_json="$2"
    local ambiguous_message="$3"
    local match_count

    match_count="$(jq 'length' <<<"$matches_json")"
    case "$match_count" in
        0) printf -v "$output_var" '%s' "" ;;
        1) printf -v "$output_var" '%s' "$(jq -c '.[0]' <<<"$matches_json")" ;;
        *) die "$ambiguous_message" ;;
    esac
}

build_plan() {
    local state_json="$1"
    local actions='[]'
    local team_key team_node team_id workspace_templates team_templates team_nodes_json

    team_key="$(jq -r '.team.key' "$CONFIG_PATH")"
    team_nodes_json="$(jq -c --arg key "$team_key" '[.data.teams.nodes[] | select(.key == $key)]' <<<"$state_json")"
    require_unique_match \
        team_node \
        "$team_nodes_json" \
        "Could not find Linear team with key '${team_key}'" \
        "Ambiguous match: found multiple teams with key '${team_key}'"

    team_id="$(jq -r '.id' <<<"$team_node")"

    while IFS= read -r desired_label; do
        [[ -n "$desired_label" ]] || continue
        local name existing current desired_input update_input existing_labels_json
        name="$(jq -r '.name' <<<"$desired_label")"
        existing_labels_json="$(jq -c --arg name "$name" '[.labels.nodes[] | select(.name == $name)]' <<<"$team_node")"
        optional_unique_match \
            existing \
            "$existing_labels_json" \
            "Ambiguous match: found multiple labels named '${name}' in team '${team_key}'"
        desired_input="$(jq -c --arg team_id "$team_id" '
            {
              teamId: $team_id,
              name: .name,
              color: .color,
              description: (.description // null),
              isGroup: (.isGroup // false)
            }' <<<"$desired_label")"

        if [[ -z "$existing" ]]; then
            actions="$(append_action "$actions" "$(jq -nc \
                --arg name "$name" \
                --arg scope "$team_key" \
                --argjson input "$desired_input" \
                '{kind:"label.create", scope:$scope, name:$name, input:$input}')")"
            continue
        fi

        current="$(jq -c '{name, color, description, isGroup}' <<<"$existing")"
        update_input="$(jq -c '{name, color, description, isGroup}' <<<"$desired_input")"
        if ! json_equals "$current" "$update_input"; then
            actions="$(append_action "$actions" "$(jq -nc \
                --arg id "$(jq -r '.id' <<<"$existing")" \
                --arg name "$name" \
                --arg scope "$team_key" \
                --argjson input "$update_input" \
                '{kind:"label.update", scope:$scope, id:$id, name:$name, input:$input}')")"
        fi
    done < <(jq -c '.team.labels[]?' "$CONFIG_PATH")

    while IFS= read -r desired_state; do
        [[ -n "$desired_state" ]] || continue
        local state_name state_type existing_state create_input current_state update_state existing_states_json
        state_name="$(jq -r '.name' <<<"$desired_state")"
        state_type="$(jq -r '.type' <<<"$desired_state")"
        existing_states_json="$(jq -c --arg name "$state_name" --arg type "$state_type" \
            '[.states.nodes[] | select(.name == $name and .type == $type)]' <<<"$team_node")"
        optional_unique_match \
            existing_state \
            "$existing_states_json" \
            "Ambiguous match: found multiple workflow states named '${state_name}' of type '${state_type}' in team '${team_key}'"
        create_input="$(jq -c --arg team_id "$team_id" '
            {
              teamId: $team_id,
              name: .name,
              type: .type,
              color: (.color // null),
              description: (.description // null),
              position: .position
            }' <<<"$desired_state")"

        if [[ -z "$existing_state" ]]; then
            actions="$(append_action "$actions" "$(jq -nc \
                --arg name "$state_name" \
                --arg type "$state_type" \
                --arg scope "$team_key" \
                --argjson input "$create_input" \
                '{kind:"workflow_state.create", scope:$scope, name:$name, type:$type, input:$input}')")"
            continue
        fi

        current_state="$(jq -c '{name, color, description, position}' <<<"$existing_state")"
        update_state="$(jq -c '{name, color, description, position}' <<<"$create_input")"
        if ! json_equals "$current_state" "$update_state"; then
            actions="$(append_action "$actions" "$(jq -nc \
                --arg id "$(jq -r '.id' <<<"$existing_state")" \
                --arg name "$state_name" \
                --arg type "$state_type" \
                --arg scope "$team_key" \
                --argjson input "$update_state" \
                '{kind:"workflow_state.update", scope:$scope, id:$id, name:$name, type:$type, input:$input}')")"
        fi
    done < <(jq -c '.team.workflowStates[]?' "$CONFIG_PATH")

    local desired_settings current_settings
    desired_settings="$(jq -c '.team.settings' "$CONFIG_PATH")"
    current_settings="$(jq -c '{
        triageEnabled,
        cyclesEnabled,
        cycleDuration,
        cycleCooldownTime,
        upcomingCycleCount
      }' <<<"$team_node")"
    if ! json_equals "$current_settings" "$desired_settings"; then
        actions="$(append_action "$actions" "$(jq -nc \
            --arg id "$team_id" \
            --arg scope "$team_key" \
            --argjson input "$(jq -c --arg id "$team_id" '. + {id: $id}' <<<"$desired_settings")" \
            '{kind:"team.update", scope:$scope, id:$id, input:$input}')")"
    fi

    workspace_templates="$(jq -c '.data.viewer.organization.templates.nodes // []' <<<"$state_json")"
    team_templates="$(jq -c '.templates.nodes // []' <<<"$team_node")"

    while IFS= read -r desired_template; do
        [[ -n "$desired_template" ]] || continue
        local template_name template_type existing_template create_template update_template existing_templates_json
        template_name="$(jq -r '.name' <<<"$desired_template")"
        template_type="$(jq -r '.type' <<<"$desired_template")"
        existing_templates_json="$(jq -c --arg name "$template_name" --arg type "$template_type" \
            '[.[] | select(.name == $name and .type == $type and (.team == null))]' <<<"$workspace_templates")"
        optional_unique_match \
            existing_template \
            "$existing_templates_json" \
            "Ambiguous match: found multiple workspace templates named '${template_name}' of type '${template_type}'"
        create_template="$(jq -c '{
            name,
            description: (.description // null),
            sortOrder: (.sortOrder // 0),
            type,
            templateData
          }' <<<"$desired_template")"

        if [[ -z "$existing_template" ]]; then
            actions="$(append_action "$actions" "$(jq -nc \
                --arg name "$template_name" \
                --arg scope "workspace" \
                --arg type "$template_type" \
                --argjson input "$create_template" \
                '{kind:"template.create", scope:$scope, name:$name, type:$type, input:$input}')")"
            continue
        fi

        update_template="$(jq -c '{
            name,
            description: (.description // null),
            sortOrder: (.sortOrder // 0)
          }' <<<"$desired_template")"
        current="$(jq -c '{name, description, sortOrder}' <<<"$existing_template")"
        if ! json_equals "$current" "$update_template"; then
            actions="$(append_action "$actions" "$(jq -nc \
                --arg id "$(jq -r '.id' <<<"$existing_template")" \
                --arg name "$template_name" \
                --arg scope "workspace" \
                --arg type "$template_type" \
                --argjson input "$update_template" \
                '{kind:"template.update", scope:$scope, id:$id, name:$name, type:$type, input:$input}')")"
        fi
    done < <(jq -c '.workspace.templates[]?' "$CONFIG_PATH")

    while IFS= read -r desired_template; do
        [[ -n "$desired_template" ]] || continue
        local template_name template_type existing_template create_template update_template existing_templates_json
        template_name="$(jq -r '.name' <<<"$desired_template")"
        template_type="$(jq -r '.type' <<<"$desired_template")"
        existing_templates_json="$(jq -c --arg name "$template_name" --arg type "$template_type" --arg team_key "$team_key" \
            '[.[] | select(.name == $name and .type == $type and .team.key == $team_key)]' <<<"$team_templates")"
        optional_unique_match \
            existing_template \
            "$existing_templates_json" \
            "Ambiguous match: found multiple templates named '${template_name}' of type '${template_type}' in team '${team_key}'"
        create_template="$(jq -c --arg team_id "$team_id" '{
            teamId: $team_id,
            name,
            description: (.description // null),
            sortOrder: (.sortOrder // 0),
            type,
            templateData
          }' <<<"$desired_template")"

        if [[ -z "$existing_template" ]]; then
            actions="$(append_action "$actions" "$(jq -nc \
                --arg name "$template_name" \
                --arg scope "$team_key" \
                --arg type "$template_type" \
                --argjson input "$create_template" \
                '{kind:"template.create", scope:$scope, name:$name, type:$type, input:$input}')")"
            continue
        fi

        update_template="$(jq -c --arg team_id "$team_id" '{
            teamId: $team_id,
            name,
            description: (.description // null),
            sortOrder: (.sortOrder // 0)
          }' <<<"$desired_template")"
        current="$(jq -c '{teamId: .team.id, name, description, sortOrder}' <<<"$existing_template")"
        if ! json_equals "$current" "$update_template"; then
            actions="$(append_action "$actions" "$(jq -nc \
                --arg id "$(jq -r '.id' <<<"$existing_template")" \
                --arg name "$template_name" \
                --arg scope "$team_key" \
                --arg type "$template_type" \
                --argjson input "$update_template" \
                '{kind:"template.update", scope:$scope, id:$id, name:$name, type:$type, input:$input}')")"
        fi
    done < <(jq -c '.team.templates[]?' "$CONFIG_PATH")

    printf '%s\n' "$actions"
}

print_plan() {
    local actions_json="$1"
    local total
    total="$(jq 'length' <<<"$actions_json")"
    echo "Plan actions: ${total}"

    jq -r '.[] | "- \(.kind) [\(.scope)] \(.name // .id)"' <<<"$actions_json"
}

apply_actions() {
    local actions_json="$1"
    local action kind response

    while IFS= read -r action; do
        [[ -n "$action" ]] || continue
        kind="$(jq -r '.kind' <<<"$action")"

        case "$kind" in
            label.create)
                linear_request 'mutation IssueLabelCreate($input: IssueLabelCreateInput!) {
  issueLabelCreate(input: $input) {
    success
    issueLabel { id name }
  }
}' "$(jq -c '{input: .input}' <<<"$action")" >/dev/null
                ;;
            label.update)
                linear_request 'mutation IssueLabelUpdate($id: String!, $input: IssueLabelUpdateInput!) {
  issueLabelUpdate(id: $id, input: $input) {
    success
    issueLabel { id name }
  }
}' "$(jq -c '{id: .id, input: .input}' <<<"$action")" >/dev/null
                ;;
            workflow_state.create)
                linear_request 'mutation WorkflowStateCreate($input: WorkflowStateCreateInput!) {
  workflowStateCreate(input: $input) {
    success
    workflowState { id name }
  }
}' "$(jq -c '{input: .input}' <<<"$action")" >/dev/null
                ;;
            workflow_state.update)
                linear_request 'mutation WorkflowStateUpdate($id: String!, $input: WorkflowStateUpdateInput!) {
  workflowStateUpdate(id: $id, input: $input) {
    success
    workflowState { id name }
  }
}' "$(jq -c '{id: .id, input: .input}' <<<"$action")" >/dev/null
                ;;
            team.update)
                linear_request 'mutation TeamUpdate($input: TeamUpdateInput!) {
  teamUpdate(input: $input) {
    success
    team { id name }
  }
}' "$(jq -c '{input: .input}' <<<"$action")" >/dev/null
                ;;
            template.create)
                linear_request 'mutation TemplateCreate($input: TemplateCreateInput!) {
  templateCreate(input: $input) {
    success
    template { id name }
  }
}' "$(jq -c '{input: .input}' <<<"$action")" >/dev/null
                ;;
            template.update)
                linear_request 'mutation TemplateUpdate($id: String!, $input: TemplateUpdateInput!) {
  templateUpdate(id: $id, input: $input) {
    success
    template { id name }
  }
}' "$(jq -c '{id: .id, input: .input}' <<<"$action")" >/dev/null
                ;;
            *)
                die "Unsupported action kind: ${kind}"
                ;;
        esac
    done < <(jq -c '.[]' <<<"$actions_json")
}

MODE="${1:-}"
shift || true

while [[ $# -gt 0 ]]; do
    case "$1" in
        --config)
            CONFIG_PATH="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            die "Unknown argument: $1"
            ;;
    esac
done

[[ -n "$MODE" ]] || { usage; exit 1; }
require_command jq
require_command curl
[[ -f "$CONFIG_PATH" ]] || die "Config file not found: ${CONFIG_PATH}"

case "$MODE" in
    validate)
        validate_config
        echo "Config valid: ${CONFIG_PATH}"
        ;;
    plan|apply)
        [[ -n "${LINEAR_API_KEY:-}" ]] || die "LINEAR_API_KEY is required for ${MODE}"
        validate_config
        state_json="$(fetch_state)"
        actions_json="$(build_plan "$state_json")"
        print_plan "$actions_json"
        if [[ "$MODE" == "apply" ]]; then
            apply_actions "$actions_json"
            echo "Applied $(jq 'length' <<<"$actions_json") actions."
        fi
        ;;
    *)
        usage
        exit 1
        ;;
esac
