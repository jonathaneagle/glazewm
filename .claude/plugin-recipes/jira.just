# Recipes provided by jira
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/jira  → absolute path to the plugin directory
#   jira  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/jira" ]; then
        echo ""
        echo "ERROR: Plugin 'jira' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/jira"
        echo ""
        echo "  To reinstall:   /plugin install jira"
        echo "  To clean up:    just jira-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
jira-uninstall:
    @rm -f ".claude/plugin-recipes/jira.just"
    @echo "Removed jira recipes from .claude/plugin-recipes/"

jira-setup: plugin-check
    #!/bin/bash
    echo "Installing jira prerequisites..."
    pip install --user keyring atlassian-python-api 2>/dev/null || true
    command -v jq &>/dev/null || { echo "Install jq: winget install jqlang.jq"; exit 1; }
    command -v acli &>/dev/null || { echo "Install acli: see https://bobswift.atlassian.net/wiki/spaces/ACLI"; exit 1; }
    echo "Done. Run 'just jira-login' to authenticate."

jira-login: plugin-check
    #!/bin/bash
    echo "Authenticating with Jira..."
    bash "/d/Repos/dev-environment/skills/jira/scripts/jira-token.sh"
    acli jira auth login --site "${CLAUDE_PLUGIN_OPTION_JIRA_INSTANCE:-datatorque.atlassian.net}" \
      --email "${CLAUDE_PLUGIN_OPTION_USER_EMAIL:-}" \
      --token < "${XDG_CACHE_HOME:-$HOME/.cache}/jira/session-token"
    echo "Jira auth complete."

jira-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/jira/scripts/check-prereqs.sh" && echo "jira: all prerequisites present"

jira-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/jira/tests/" -m "not integration" -v
