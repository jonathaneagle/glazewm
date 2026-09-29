# Recipes provided by git
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/git  → absolute path to the plugin directory
#   git  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/git" ]; then
        echo ""
        echo "ERROR: Plugin 'git' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/git"
        echo ""
        echo "  To reinstall:   /plugin install git"
        echo "  To clean up:    just git-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
git-uninstall:
    @rm -f ".claude/plugin-recipes/git.just"
    @echo "Removed git recipes from .claude/plugin-recipes/"

git-setup: plugin-check
    #!/bin/bash
    echo "Checking git..."
    command -v git &>/dev/null && echo "git $(git --version | cut -d' ' -f3) is installed." || { echo "Install git: winget install Git.Git"; exit 1; }

git-login: plugin-check
    #!/bin/bash
    echo "Git uses GCM (Git Credential Manager) — no manual login needed."
    git credential-manager --version 2>/dev/null && echo "GCM is configured." || echo "GCM not found. Reinstall Git for Windows with GCM enabled."

git-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/git/scripts/check-prereqs.sh" && echo "git: all prerequisites present"

git-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/git/tests/" -m "not integration" -v
