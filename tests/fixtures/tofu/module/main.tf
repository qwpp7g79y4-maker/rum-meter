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

module "child" {
  source = "./child"
}
