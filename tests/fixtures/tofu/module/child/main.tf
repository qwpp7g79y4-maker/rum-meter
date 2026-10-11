terraform {
  required_providers {
    random = {
      source = "hashicorp/random"
    }
  }
}

resource "random_id" "one" {
  byte_length = 16
  prefix      = "canary-module-child-7f3a9c2e1b04d8aa"
}

module "nested" {
  source = "./nested"
}
