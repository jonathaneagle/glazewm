# Recipes provided by powerpoint
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/powerpoint  → absolute path to the plugin directory
#   powerpoint  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/powerpoint" ]; then
        echo ""
        echo "ERROR: Plugin 'powerpoint' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/powerpoint"
        echo ""
        echo "  To reinstall:   /plugin install powerpoint"
        echo "  To clean up:    just powerpoint-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
powerpoint-uninstall:
    @rm -f ".claude/plugin-recipes/powerpoint.just"
    @echo "Removed powerpoint recipes from .claude/plugin-recipes/"

powerpoint-setup: plugin-check
    #!/bin/bash
    echo "Checking PowerPoint prerequisites..."
    command -v pwsh &>/dev/null || { echo "Install PowerShell: winget install Microsoft.PowerShell"; exit 1; }
    echo "PowerPoint MCP server is required. Run: just install-mcp-servers"

powerpoint-login: plugin-check
    @echo "PowerPoint does not require authentication."

powerpoint-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/powerpoint/scripts/check-prereqs.sh" && echo "powerpoint: all prerequisites present"

powerpoint-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/powerpoint/tests/" -m "not integration" -v
