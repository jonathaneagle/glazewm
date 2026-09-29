# Recipes provided by ghe
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/ghe  → absolute path to the plugin directory
#   ghe  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/ghe" ]; then
        echo ""
        echo "ERROR: Plugin 'ghe' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/ghe"
        echo ""
        echo "  To reinstall:   /plugin install ghe"
        echo "  To clean up:    just ghe-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
ghe-uninstall:
    @rm -f ".claude/plugin-recipes/ghe.just"
    @echo "Removed ghe recipes from .claude/plugin-recipes/"

ghe-setup: plugin-check
    #!/bin/bash
    echo "Installing ghe prerequisites..."
    command -v gh &>/dev/null || { echo "Install gh: winget install GitHub.cli"; exit 1; }
    command -v jq &>/dev/null || { echo "Install jq: winget install jqlang.jq"; exit 1; }
    command -v pwsh &>/dev/null || { echo "Install PowerShell: winget install Microsoft.PowerShell"; exit 1; }
    echo "Done. Run 'just ghe-login' to authenticate."

ghe-login: plugin-check
    #!/bin/bash
    GHE_HOST="${CLAUDE_PLUGIN_OPTION_GHE_HOST:-datatorque.ghe.com}"
    echo "Authenticating with GHE at $GHE_HOST..."
    gh auth login --hostname "$GHE_HOST"

ghe-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/ghe/scripts/check-prereqs.sh" && echo "ghe: all prerequisites present"

ghe-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/ghe/tests/" -m "not integration" -v
