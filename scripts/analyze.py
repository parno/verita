#!/usr/bin/env python

import argparse
import json
import glob

class Project:
    def __init__(self, json):
        self.name = json["runner"]["run_configuration"]["name"]
        self.refspec = json["runner"]["run_configuration"]["refspec"]
        self.times_ms = json["times-ms"]

    def __str__(self):
        return f'{self.name} <self.refspec>'

def read_json_files_into_projects(directory):
    projects = []
    for filename in glob.glob(f'{directory}/*.json'):
        with open(filename, 'r') as file:
            projects.append(Project(json.load(file)))
    return projects



def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--dir', required=True, help='Directory of results to analyze')
    args = parser.parse_args()

    projects = read_json_files_into_projects(args.dir)

if __name__ == '__main__':
    main()