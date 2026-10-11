terraform {
  required_version = ">= 1.13.0"

  backend "local" {}

  required_providers {
    random = {
      source  = "hashicorp/random"
      version = "3.9.1"
    }
  }
}

resource "random_id" "servers" {
  count       = 3
  byte_length = 16
  prefix      = "canary-count-servers-7f3a9c2e1b04d8aa"
}

resource "random_password" "secret" {
  length           = 32
  special          = true
  override_special = "!#%*-_=+"
}
