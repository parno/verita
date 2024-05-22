#!/usr/bin/env python

import argparse
import json
import glob
from matplotlib import pyplot as plt
from matplotlib.backends.backend_pdf import PdfPages
import numpy as np
import os

def go(filename):
    with open(filename, 'r') as file:
        with open(filename, 'r') as file:
            stderr = json.load(file)["runner"]["stderr"]
            print(stderr)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('filename', help='File to extract stderr')
    args = parser.parse_args()

    go(args.filename)

if __name__ == '__main__':
    main()