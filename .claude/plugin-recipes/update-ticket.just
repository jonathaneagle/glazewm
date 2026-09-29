# Recipes provided by update-ticket
# Synced by SessionStart hook. Placeholders are replaced by sync-justfile.sh:
#   /d/Repos/dev-environment/skills/update-ticket  → absolute path to the plugin directory
#   update-ticket  → plugin name

# Verify the plugin is still installed before running recipes
[private]
plugin-check:
    #!/bin/bash
    if [ ! -d "/d/Repos/dev-environment/skills/update-ticket" ]; then
        echo ""
        echo "ERROR: Plugin 'update-ticket' is not installed."
        echo "Expected at: /d/Repos/dev-environment/skills/update-ticket"
        echo ""
        echo "  To reinstall:   /plugin install update-ticket"
        echo "  To clean up:    just update-ticket-uninstall"
        echo ""
        exit 1
    fi

# Remove this recipe file (use after uninstalling the plugin)
update-ticket-uninstall:
    @rm -f ".claude/plugin-recipes/update-ticket.just"
    @echo "Removed update-ticket recipes from .claude/plugin-recipes/"

update-ticket-setup: plugin-check
    #!/bin/bash
    echo "update-ticket inherits tools from jira."
    echo "Run 'just jira-setup' if needed."

update-ticket-login: plugin-check
    @echo "update-ticket uses auth from jira. Run 'just jira-login'."

update-ticket-check: plugin-check
    @bash "/d/Repos/dev-environment/skills/update-ticket/scripts/check-prereqs.sh" && echo "update-ticket: all prerequisites present"

update-ticket-test: plugin-check
    pytest "/d/Repos/dev-environment/skills/update-ticket/tests/" -m "not integration" -v
