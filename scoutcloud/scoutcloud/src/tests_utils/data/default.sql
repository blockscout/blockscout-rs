-- Inserting mock data for users
INSERT INTO "users" ("id", "email", "project_title")
VALUES
    (1, 'user1@example.com', 'Project 1'),
    (2, 'user2@example.com', 'Project 2');

INSERT INTO "balance_changes" ("user_id", "amount", "note") VALUES
    (1, 100, 'Balance deposit'),
    (2, 20, NULL);

-- Inserting mock data for auth_keys
INSERT INTO "auth_tokens" ("user_id")
VALUES
    (1),
    (2);

-- Inserting mock data for instances
INSERT INTO "instances" ("id", "creator_id", "name", "slug", "user_config", "parsed_config")
VALUES
    (1, 1, 'Instance 1', 'instance-1', '{"rpc_url": "https://sepolia.drpc.org/","node_type": "geth", "chain_type": "ethereum", "server_size": "medium"}', '{}'),
    (2, 2, 'Instance 2', 'instance-2', '{"rpc_url": "https://sepolia.drpc.org/","node_type": "geth", "chain_type": "ethereum", "server_size": "medium"}', '{}'),
    (3, 2, 'Instance 3', 'instance-3', '{"rpc_url": "https://sepolia.drpc.org/","node_type": "geth", "chain_type": "ethereum", "server_size": "medium"}', '{}');


-- Inserting mock data for deployments
INSERT INTO "deployments" ("id", "instance_id", "server_spec_id", "created_at", "started_at", "status", "user_config", "parsed_config")
VALUES
    (1, 1, 1, NOW() - INTERVAL '5 hours', NOW() - INTERVAL '4 hours', 'running', '{}', '{"frontend": {"ingress": {"hostname": "instance.example.com"}}}'),
    (2, 2, 1, NOW() - INTERVAL '4 hours', NOW() - INTERVAL '3 hours', 'stopped', '{}', '{"frontend": {"ingress": {"hostname": "instance.example.com"}}}'),
    (3, 2, 1, NOW() - INTERVAL '3 hours', NOW() - INTERVAL '2 hours', 'failed', '{}', '{"frontend": {"ingress": {"hostname": "instance.example.com"}}}'),
    (4, 3, 1, NOW() - INTERVAL '2 hours', NULL, 'created', '{}', '{"frontend": {"ingress": {"hostname": "instance.example.com"}}}');
