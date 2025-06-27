# Terraform Configuration for HashiCorp Vault

This Terraform configuration sets up HashiCorp Vault for secure Omikuji key storage.

## Prerequisites

- Terraform >= 1.0
- HashiCorp Vault >= 1.12
- Vault provider configured with admin credentials

## Usage

1. **Initialize Terraform:**
   ```bash
   terraform init
   ```

2. **Configure variables:**
   Create a `terraform.tfvars` file:
   ```hcl
   vault_address = "https://vault.mycompany.com:8200"
   omikuji_environments = ["dev", "staging", "prod"]
   ```

3. **Plan and apply:**
   ```bash
   terraform plan
   terraform apply
   ```

4. **Get the AppRole credentials:**
   ```bash
   # Get Role ID (not secret)
   terraform output approle_role_id
   
   # Generate a Secret ID (do this from your deployment pipeline)
   vault write -f auth/approle/role/omikuji/secret-id
   ```

5. **Configure Omikuji:**
   ```yaml
   key_storage:
     backend: "vault"
     vault:
       url: "https://vault.mycompany.com:8200"
       mount_path: "omikuji"
       path_prefix: "keys"
       auth_method: "token"
       token: "${VAULT_TOKEN}"
       cache_ttl_seconds: 300
   ```

## What This Creates

- **KV v2 Secrets Engine** at path `omikuji/`
- **Vault Policy** with appropriate permissions
- **AppRole** for production authentication
- **Namespaces** for environment isolation (optional)
- **Audit Logging** to track all key operations

## Security Notes

1. Store the Terraform state file securely (e.g., in S3 with encryption)
2. Never commit Secret IDs to version control
3. Rotate Secret IDs regularly
4. Use separate Vault instances or namespaces for different environments
5. Enable TLS certificate verification in production

## Next Steps

1. Generate Secret IDs in your CI/CD pipeline
2. Configure Omikuji with the Vault backend
3. Import existing keys if migrating from another backend
4. Set up monitoring for the audit logs