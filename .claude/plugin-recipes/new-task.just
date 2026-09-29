# Recipes provided by new-task
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/new-task  → absolute path to the plugin directory
#   new-task  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/new-task" ]; then
        echo ""
        echo "ERROR: Plugin 'new-task' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/new-task"
        echo ""
        echo "  To reinstall:   /plugin install new-task"
        echo "  To clean up:    just new-task-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
new-task-uninstall:
    @rm -f ".claude/plugin-recipes/new-task.just"
    @echo "Removed new-task recipes from .claude/plugin-recipes/"

new-task-setup: plugin-check
    #!/bin/bash
    echo "new-task inherits tools from jira and git."
    echo "Run 'just jira-setup' and 'just git-setup' if needed."

new-task-login: plugin-check
    @echo "new-task uses auth from jira. Run 'just jira-login'."

new-task-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/new-task/scripts/check-prereqs.sh" && echo "new-task: all prerequisites present"

new-task-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/new-task/tests/" -m "not integration" -v
