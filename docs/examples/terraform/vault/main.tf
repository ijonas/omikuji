# HashiCorp Vault Setup for Omikuji

variable "vault_address" {
  description = "The address of the Vault server"
  type        = string
  default     = "https://vault.example.com:8200"
}

variable "omikuji_environments" {
  description = "List of Omikuji environments (e.g., dev, staging, prod)"
  type        = list(string)
  default     = ["dev", "staging", "prod"]
}

# Enable KV v2 secrets engine
resource "vault_mount" "omikuji_secrets" {
  path        = "omikuji"
  type        = "kv-v2"
  description = "KV v2 secrets engine for Omikuji private keys"
  
  options = {
    version = "2"
  }
}

# Create policy for Omikuji
resource "vault_policy" "omikuji" {
  name = "omikuji"

  policy = <<EOT
# Read and write secrets
path "omikuji/data/*" {
  capabilities = ["create", "read", "update", "delete", "list"]
}

# List secrets
path "omikuji/metadata/*" {
  capabilities = ["list", "read", "delete"]
}

# Read own token info (for health checks)
path "auth/token/lookup-self" {
  capabilities = ["read"]
}
EOT
}

# Create AppRole for production use
resource "vault_auth_backend" "approle" {
  type = "approle"
  path = "approle"
}

resource "vault_approle_auth_backend_role" "omikuji" {
  backend        = vault_auth_backend.approle.path
  role_name      = "omikuji"
  token_policies = [vault_policy.omikuji.name]
  
  # Token settings
  token_ttl     = 3600  # 1 hour
  token_max_ttl = 14400 # 4 hours
  
  # Security settings
  secret_id_ttl       = 86400 # 24 hours
  secret_id_num_uses  = 0     # Unlimited uses
  bind_secret_id      = true
  token_explicit_max_ttl = 0
}

# Create namespaces for different environments
resource "vault_namespace" "omikuji_env" {
  for_each = toset(var.omikuji_environments)
  path     = "omikuji-${each.key}"
}

# Enable audit logging
resource "vault_audit" "file" {
  type = "file"
  
  options = {
    file_path = "/vault/logs/audit.log"
  }
}

# Output the role ID (not sensitive)
output "approle_role_id" {
  value = vault_approle_auth_backend_role.omikuji.role_id
}

# Output mount path
output "secrets_mount_path" {
  value = vault_mount.omikuji_secrets.path
}

# Example secret creation (optional - Omikuji can create these itself)
resource "vault_kv_secret_v2" "example_mainnet_key" {
  mount = vault_mount.omikuji_secrets.path
  name  = "keys/ethereum-mainnet"
  
  data_json = jsonencode({
    private_key = "0x..." # Replace with actual key
    network     = "ethereum-mainnet"
    created_at  = timestamp()
    created_by  = "terraform"
  })
  
  lifecycle {
    ignore_changes = [data_json] # Don't overwrite if changed externally
  }
}