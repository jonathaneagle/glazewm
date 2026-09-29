# Recipes provided by implement-task
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/implement-task  → absolute path to the plugin directory
#   implement-task  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/implement-task" ]; then
        echo ""
        echo "ERROR: Plugin 'implement-task' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/implement-task"
        echo ""
        echo "  To reinstall:   /plugin install implement-task"
        echo "  To clean up:    just implement-task-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
implement-task-uninstall:
    @rm -f ".claude/plugin-recipes/implement-task.just"
    @echo "Removed implement-task recipes from .claude/plugin-recipes/"

implement-task-setup: plugin-check
    #!/bin/bash
    echo "implement-task inherits tools from jira and git."
    echo "Run 'just jira-setup' and 'just git-setup' if needed."

implement-task-login: plugin-check
    @echo "implement-task uses auth from jira. Run 'just jira-login'."

implement-task-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/implement-task/scripts/check-prereqs.sh" && echo "implement-task: all prerequisites present"

implement-task-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/implement-task/tests/" -m "not integration" -v
