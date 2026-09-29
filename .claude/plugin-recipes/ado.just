# Recipes provided by ado
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/ado  → absolute path to the plugin directory
#   ado  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/ado" ]; then
        echo ""
        echo "ERROR: Plugin 'ado' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/ado"
        echo ""
        echo "  To reinstall:   /plugin install ado"
        echo "  To clean up:    just ado-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
ado-uninstall:
    @rm -f ".claude/plugin-recipes/ado.just"
    @echo "Removed ado recipes from .claude/plugin-recipes/"

ado-setup: plugin-check
    #!/bin/bash
    echo "Installing ado prerequisites..."
    command -v jq &>/dev/null || { echo "Install jq: winget install jqlang.jq"; exit 1; }
    command -v keyring.exe &>/dev/null || { echo "Install keyring: pip install keyring"; exit 1; }
    command -v pwsh &>/dev/null || { echo "Install PowerShell: winget install Microsoft.PowerShell"; exit 1; }
    echo "Done. Run 'just ado-login' to store your PAT."

ado-login: plugin-check
    #!/bin/bash
    echo "Store your ADO Personal Access Token..."
    read -rsp "Enter PAT: " pat && echo
    keyring.exe set ado-pat "$USER" <<< "$pat"
    echo "PAT stored in Windows Credential Manager."

ado-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/ado/scripts/check-prereqs.sh" && echo "ado: all prerequisites present"

ado-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/ado/tests/" -m "not integration" -v
