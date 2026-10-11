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

variable "keeper" {
  type    = string
  default = "canary-deposed-keeper-first-7f3a9c2e1b04d8aa"
}

resource "random_pet" "replaced" {
  keepers = {
    step = var.keeper
  }
  length    = 2
  separator = "canarysep7f3a9c2e1b04"
  prefix    = "canary-deposed-pet-7f3a9c2e1b04d8aa"

  lifecycle {
    create_before_destroy = true
  }

  # OpenTofu 1.13.1 skips the destroy-time provisioner on a deposed object.
  # This create-time provisioner fails on the second apply, after the
  # previous object is already deposed.
  provisioner "local-exec" {
    command = "test -f ${path.module}/allow-create.generated"
  }
}
