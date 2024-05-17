#!/usr/bin/env python

import argparse
import json
import glob

class FunctionSmtTime:
    def __init__(self, json):
        self.name = json["function"]
        self.time_ms = json["time"]

    def __str__(self):
        return f'{self.name} <{self.time_ms}>'

class Project:
    def __init__(self, json):
        self.name = json["runner"]["run_configuration"]["name"]
        self.refspec = json["runner"]["run_configuration"]["refspec"]
        self.times_ms = json["times-ms"]

        # Collect SMT times
        self.fn_smt_times = []
        for item in self.times_ms["smt"]["smt-run-module-times"]:
            for function in item["function-breakdown"]:
                self.fn_smt_times.append(FunctionSmtTime(function))

    def __str__(self):
        return f'{self.name} <{self.refspec}>'

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