# Recipes provided by pr
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/pr  → absolute path to the plugin directory
#   pr  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/pr" ]; then
        echo ""
        echo "ERROR: Plugin 'pr' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/pr"
        echo ""
        echo "  To reinstall:   /plugin install pr"
        echo "  To clean up:    just pr-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
pr-uninstall:
    @rm -f ".claude/plugin-recipes/pr.just"
    @echo "Removed pr recipes from .claude/plugin-recipes/"

pr-setup: plugin-check
    #!/bin/bash
    echo "Installing pr prerequisites..."
    command -v gh &>/dev/null || { echo "Install gh: winget install GitHub.cli"; exit 1; }
    command -v jq &>/dev/null || { echo "Install jq: winget install jqlang.jq"; exit 1; }
    command -v pwsh &>/dev/null || { echo "Install PowerShell: winget install Microsoft.PowerShell"; exit 1; }
    echo "Done. PR skill inherits auth from ghe and ado skills."

pr-login: plugin-check
    @echo "PR skill uses auth from ghe and ado. Run 'just ghe-login' or 'just ado-login'."

pr-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/pr/scripts/check-prereqs.sh" && echo "pr: all prerequisites present"

pr-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/pr/tests/" -m "not integration" -v
