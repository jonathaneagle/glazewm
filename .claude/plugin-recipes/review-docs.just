# Recipes provided by review-docs
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/review-docs  → absolute path to the plugin directory
#   review-docs  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/review-docs" ]; then
        echo ""
        echo "ERROR: Plugin 'review-docs' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/review-docs"
        echo ""
        echo "  To reinstall:   /plugin install review-docs"
        echo "  To clean up:    just review-docs-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
review-docs-uninstall:
    @rm -f ".claude/plugin-recipes/review-docs.just"
    @echo "Removed review-docs recipes from .claude/plugin-recipes/"

review-docs-setup: plugin-check
    @echo "review-docs uses only standard bash tools. No additional setup required."

review-docs-login: plugin-check
    @echo "review-docs does not require authentication."

review-docs-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/review-docs/scripts/check-prereqs.sh" && echo "review-docs: all prerequisites present"

review-docs-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/review-docs/tests/" -m "not integration" -v
