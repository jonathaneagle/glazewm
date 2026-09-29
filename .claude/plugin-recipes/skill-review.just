# Recipes provided by skill-review
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/skill-review  → absolute path to the plugin directory
#   skill-review  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/skill-review" ]; then
        echo ""
        echo "ERROR: Plugin 'skill-review' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/skill-review"
        echo ""
        echo "  To reinstall:   /plugin install skill-review"
        echo "  To clean up:    just skill-review-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
skill-review-uninstall:
    @rm -f ".claude/plugin-recipes/skill-review.just"
    @echo "Removed skill-review recipes from .claude/plugin-recipes/"

skill-review-setup: plugin-check
    @echo "skill-review uses only standard bash tools. No additional setup required."

skill-review-login: plugin-check
    @echo "skill-review does not require authentication."

skill-review-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/skill-review/scripts/check-prereqs.sh" && echo "skill-review: all prerequisites present"

skill-review-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/skill-review/tests/" -m "not integration" -v
