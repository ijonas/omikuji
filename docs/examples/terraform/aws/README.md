# Terraform Configuration for AWS Secrets Manager

This Terraform configuration sets up AWS Secrets Manager and related resources for secure Omikuji key storage.

## Prerequisites

- Terraform >= 1.0
- AWS CLI configured with appropriate credentials
- Sufficient AWS permissions to create IAM roles, KMS keys, and Secrets Manager secrets

## Usage

1. **Initialize Terraform:**
   ```bash
   terraform init
   ```

2. **Configure variables:**
   Create a `terraform.tfvars` file:
   ```hcl
   aws_region = "us-east-1"
   environment = "prod"
   omikuji_networks = ["ethereum-mainnet", "base-mainnet", "arbitrum-mainnet"]
   ```

3. **Plan and apply:**
   ```bash
   terraform plan
   terraform apply
   ```

4. **Configure Omikuji:**
   ```yaml
   key_storage:
     backend: "aws-secrets"
     aws_secrets:
       region: "us-east-1"
       prefix: "omikuji"
       cache_ttl_seconds: 300
   ```

## What This Creates

### Security Resources
- **KMS Key** - Customer-managed key for encrypting secrets at rest
- **IAM Policies** - Least-privilege access to secrets
- **IAM Roles** - For EC2 instances and ECS tasks

### Secrets Management
- **Secrets Manager** - Stores encrypted private keys
- **Resource Tags** - For organization and cost allocation

### Audit & Compliance
- **CloudTrail** - Logs all secret access operations
- **S3 Bucket** - Stores CloudTrail logs with lifecycle policies

## Deployment Scenarios

### EC2 Instance

Attach the instance profile when launching EC2:
```bash
aws ec2 run-instances \
  --iam-instance-profile Name=$(terraform output -raw instance_profile_name) \
  ...
```

### ECS Task

Use the task role in your task definition:
```json
{
  "taskRoleArn": "$(terraform output -raw task_role_arn)",
  "executionRoleArn": "...",
  ...
}
```

### Lambda Function

Create an execution role with the secrets policy attached:
```hcl
resource "aws_iam_role_policy_attachment" "lambda_secrets" {
  role       = aws_iam_role.lambda_execution.name
  policy_arn = aws_iam_policy.omikuji_secrets.arn
}
```

## Managing Secrets

### Create a Secret
```bash
aws secretsmanager create-secret \
  --name "omikuji/ethereum-mainnet" \
  --kms-key-id "alias/omikuji-prod" \
  --secret-string '{
    "private_key": "0x...",
    "network": "ethereum-mainnet",
    "created_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "created_by": "admin"
  }'
```

### Update a Secret
```bash
aws secretsmanager put-secret-value \
  --secret-id "omikuji/ethereum-mainnet" \
  --secret-string '{"private_key": "0x..."}'
```

### Rotate Secrets
1. Create a Lambda function for rotation
2. Configure automatic rotation:
   ```bash
   aws secretsmanager rotate-secret \
     --secret-id "omikuji/ethereum-mainnet" \
     --rotation-lambda-arn "arn:aws:lambda:..."
   ```

## Security Best Practices

1. **Enable MFA Delete** on the CloudTrail S3 bucket
2. **Use VPC Endpoints** for Secrets Manager access
3. **Enable GuardDuty** to detect unusual API activity
4. **Set up CloudWatch Alarms** for failed secret retrievals
5. **Review IAM Policies** regularly and remove unused permissions

## Cost Optimization

- Secrets cost $0.40/month each
- API calls: $0.05 per 10,000 calls
- Consider caching to reduce API calls
- Delete unused secrets (7-day recovery window)
- Use resource tags for cost allocation

## Monitoring

### CloudWatch Metrics
Monitor these metrics:
- `AWS/SecretsManager/SecretCount`
- `AWS/SecretsManager/ResourceCount` 
- API call counts and errors

### Example Alarm
```hcl
resource "aws_cloudwatch_metric_alarm" "secret_access_failures" {
  alarm_name          = "omikuji-secret-access-failures"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "UserErrors"
  namespace           = "AWS/SecretsManager"
  period              = "300"
  statistic           = "Sum"
  threshold           = "5"
  alarm_description   = "Too many secret access failures"
}
```

## Cleanup

To remove all resources:
```bash
terraform destroy
```

Note: Secrets will be scheduled for deletion with a 7-day recovery window.