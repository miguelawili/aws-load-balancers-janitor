# aws-load-balancers-janitor
For cleaning up unused AWS load balancers. Supports ALBs, NLBs, Classic Load Balancers.

# TODO
- [/] Add `vpc_id` to Structs so we can add it as a filter for deletion. (Only delete if `vpc_id` is included in configuration).
- [/] Refactor deletion. (Test if working).
