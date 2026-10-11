terraform {
  required_providers {
    random = {
      source = "hashicorp/random"
    }
  }
}

resource "random_id" "pair" {
  count       = 2
  byte_length = 16
  prefix      = "canary-module-nested-7f3a9c2e1b04d8aa"
}
