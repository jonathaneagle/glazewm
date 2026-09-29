# Recipes provided by m365-suite
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/plugins/m365-suite  → absolute path to the plugin directory
#   m365-suite  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/plugins/m365-suite" ]; then
        echo ""
        echo "ERROR: Plugin 'm365-suite' is not installed."
        echo "Expected at: /d/Repos/dev-environment/plugins/m365-suite"
        echo ""
        echo "  To reinstall:   /plugin install m365-suite"
        echo "  To clean up:    just m365-suite-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
m365-suite-uninstall:
    @rm -f ".claude/plugin-recipes/m365-suite.just"
    @echo "Removed m365-suite recipes from .claude/plugin-recipes/"

m365-suite-setup: plugin-check
    #!/bin/bash
    echo "Installing m365-suite prerequisites..."
    command -v jq &>/dev/null || { echo "Install jq: winget install jqlang.jq"; exit 1; }
    command -v curl &>/dev/null || { echo "curl should be pre-installed on Windows 11."; exit 1; }
    echo "Done. Run 'just m365-suite-login' to authenticate."

m365-suite-login: plugin-check
    #!/bin/bash
    echo "Starting M365 OAuth device-code login..."
    bash "/d/Repos/dev-environment/plugins/m365-suite/_lib/m365-login.sh"

m365-suite-check: plugin-check
    @bash "/d/Repos/dev-environment/plugins/m365-suite/scripts/check-prereqs.sh" && echo "m365-suite: all prerequisites present"

m365-suite-test: plugin-check
    #!/bin/bash
    echo "Running tests for all M365 sub-skills..."
    pytest "/d/Repos/dev-environment/plugins/m365-suite/skills/calendar/tests/" "/d/Repos/dev-environment/plugins/m365-suite/skills/teams/tests/" "/d/Repos/dev-environment/plugins/m365-suite/skills/sharepoint/tests/" -m "not integration" -v
