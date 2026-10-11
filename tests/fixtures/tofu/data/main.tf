terraform {
  required_version = ">= 1.13.0"

  backend "local" {}

  required_providers {
    local = {
      source  = "hashicorp/local"
      version = "2.9.1"
    }
    null = {
      source  = "hashicorp/null"
      version = "3.3.2"
    }
    random = {
      source  = "hashicorp/random"
      version = "3.9.1"
    }
  }
}

resource "random_id" "kept" {
  byte_length = 16
  prefix      = "canary-data-kept-7f3a9c2e1b04d8aa"
}

data "local_file" "note" {
  filename = "${path.module}/note.generated"
}

resource "null_resource" "wait" {
  triggers = {
    token = "canary-null-trigger-7f3a9c2e1b04d8aa91c0"
  }
}

resource "terraform_data" "marker" {
  input = "canary-terraform-data-input-7f3a9c2e1b04d8aa91c0"
}
