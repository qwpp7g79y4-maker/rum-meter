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

resource "random_id" "labeled" {
  for_each    = toset(["web", "db", "api"])
  byte_length = 16
  prefix      = "canary-for-each-labeled-7f3a9c2e1b04d8aa"
}
