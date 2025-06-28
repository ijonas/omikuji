# AWS Secrets Manager Setup for Omikuji

variable "aws_region" {
  description = "AWS region for resources"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Environment name (e.g., dev, staging, prod)"
  type        = string
  default     = "prod"
}

variable "omikuji_networks" {
  description = "List of blockchain networks Omikuji will manage"
  type        = list(string)
  default     = ["ethereum-mainnet", "ethereum-sepolia", "base-mainnet", "base-sepolia"]
}

# KMS key for encrypting secrets
resource "aws_kms_key" "omikuji" {
  description             = "KMS key for Omikuji secrets encryption"
  deletion_window_in_days = 30
  enable_key_rotation     = true

  tags = {
    Name        = "omikuji-${var.environment}"
    Environment = var.environment
    Application = "omikuji"
  }
}

resource "aws_kms_alias" "omikuji" {
  name          = "alias/omikuji-${var.environment}"
  target_key_id = aws_kms_key.omikuji.key_id
}

# IAM policy for Omikuji
resource "aws_iam_policy" "omikuji_secrets" {
  name        = "omikuji-secrets-${var.environment}"
  description = "Policy for Omikuji to access Secrets Manager"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "secretsmanager:GetSecretValue",
          "secretsmanager:CreateSecret",
          "secretsmanager:UpdateSecret",
          "secretsmanager:DeleteSecret",
          "secretsmanager:DescribeSecret",
          "secretsmanager:TagResource"
        ]
        Resource = "arn:aws:secretsmanager:${var.aws_region}:${data.aws_caller_identity.current.account_id}:secret:omikuji/*"
      },
      {
        Effect = "Allow"
        Action = [
          "secretsmanager:ListSecrets"
        ]
        Resource = "*"
      },
      {
        Effect = "Allow"
        Action = [
          "kms:Decrypt",
          "kms:GenerateDataKey"
        ]
        Resource = aws_kms_key.omikuji.arn
      }
    ]
  })
}

# IAM role for EC2 instances running Omikuji
resource "aws_iam_role" "omikuji_instance" {
  name = "omikuji-instance-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ec2.amazonaws.com"
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "omikuji_secrets" {
  role       = aws_iam_role.omikuji_instance.name
  policy_arn = aws_iam_policy.omikuji_secrets.arn
}

resource "aws_iam_instance_profile" "omikuji" {
  name = "omikuji-${var.environment}"
  role = aws_iam_role.omikuji_instance.name
}

# IAM role for ECS tasks (if using ECS)
resource "aws_iam_role" "omikuji_task" {
  name = "omikuji-task-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ecs-tasks.amazonaws.com"
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "omikuji_task_secrets" {
  role       = aws_iam_role.omikuji_task.name
  policy_arn = aws_iam_policy.omikuji_secrets.arn
}

# Example secret creation (optional - Omikuji can create these itself)
resource "aws_secretsmanager_secret" "example_key" {
  for_each = toset(var.omikuji_networks)
  
  name                    = "omikuji/${each.key}"
  description             = "Omikuji private key for network: ${each.key}"
  kms_key_id              = aws_kms_key.omikuji.id
  recovery_window_in_days = 7

  tags = {
    Environment = var.environment
    Application = "omikuji"
    Network     = each.key
  }
}

# CloudTrail for audit logging
resource "aws_cloudtrail" "omikuji" {
  name           = "omikuji-${var.environment}"
  s3_bucket_name = aws_s3_bucket.cloudtrail.id

  event_selector {
    read_write_type           = "All"
    include_management_events = true

    data_resource {
      type   = "AWS::SecretsManager::Secret"
      values = ["arn:aws:secretsmanager:*:*:secret:omikuji/*"]
    }
  }

  tags = {
    Environment = var.environment
    Application = "omikuji"
  }
}

# S3 bucket for CloudTrail logs
resource "aws_s3_bucket" "cloudtrail" {
  bucket = "omikuji-cloudtrail-${var.environment}-${data.aws_caller_identity.current.account_id}"

  tags = {
    Environment = var.environment
    Application = "omikuji"
  }
}

resource "aws_s3_bucket_lifecycle_configuration" "cloudtrail" {
  bucket = aws_s3_bucket.cloudtrail.id

  rule {
    id     = "expire-old-logs"
    status = "Enabled"

    expiration {
      days = 90
    }
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "cloudtrail" {
  bucket = aws_s3_bucket.cloudtrail.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_s3_bucket_public_access_block" "cloudtrail" {
  bucket = aws_s3_bucket.cloudtrail.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# Data source for current AWS account
data "aws_caller_identity" "current" {}

# Outputs
output "instance_profile_name" {
  description = "Name of the IAM instance profile for EC2"
  value       = aws_iam_instance_profile.omikuji.name
}

output "task_role_arn" {
  description = "ARN of the IAM role for ECS tasks"
  value       = aws_iam_role.omikuji_task.arn
}

output "kms_key_id" {
  description = "ID of the KMS key for secrets encryption"
  value       = aws_kms_key.omikuji.id
}

output "secrets_prefix" {
  description = "Prefix for all Omikuji secrets"
  value       = "omikuji"
}