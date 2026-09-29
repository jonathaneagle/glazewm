# Recipes provided by finish-work
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/finish-work  → absolute path to the plugin directory
#   finish-work  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/finish-work" ]; then
        echo ""
        echo "ERROR: Plugin 'finish-work' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/finish-work"
        echo ""
        echo "  To reinstall:   /plugin install finish-work"
        echo "  To clean up:    just finish-work-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
finish-work-uninstall:
    @rm -f ".claude/plugin-recipes/finish-work.just"
    @echo "Removed finish-work recipes from .claude/plugin-recipes/"

finish-work-setup: plugin-check
    #!/bin/bash
    echo "finish-work inherits tools from jira and pr."
    echo "Run 'just jira-setup' and 'just pr-setup' if needed."

finish-work-login: plugin-check
    @echo "finish-work uses auth from jira and pr. Run 'just jira-login' or 'just ghe-login'."

finish-work-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/finish-work/scripts/check-prereqs.sh" && echo "finish-work: all prerequisites present"

finish-work-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/finish-work/tests/" -m "not integration" -v
