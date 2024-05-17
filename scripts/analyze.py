#!/usr/bin/env python

import argparse
import json
import glob
from matplotlib import pyplot as plt
import numpy as np
import os

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
        self.total_solved = json["verification-results"]["verified"]
        self.errors = json["verification-results"]["errors"]

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

class Run:
    def __init__(self, directory):
        self.directory = directory
        self.projects = read_json_files_into_projects(directory)

    def __str__(self):
        return f'{self.project} <{self.time_ms}>'

def plot_project_survival_curve(project):
    # Calculate survival curve
    times = [f.time_ms for f in project.fn_smt_times]
    perf = np.array(np.sort(times))
    cdf = np.cumsum(perf)
    plt.plot(cdf, np.arange(0, len(cdf)), label=project.name, linestyle="solid", color="black")
    plt.title(f"{project.name} - Solved {project.total_solved}, with {project.errors} errors")
    plt.ylim(0)
    plt.xlim(0.1)
    plt.xscale("log")
    plt.xlabel("Time Log Scale (ms)")
    plt.ylabel("Instances Soveld")
    plt.grid()
    os.makedirs("fig/survival", exist_ok=True)
    plt.savefig(f"fig/survival/{project.name}.pdf")
    plt.close()
    



def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--dir', required=True, help='Directory of results to analyze')
    args = parser.parse_args()

    run = Run(args.dir)
    for project in run.projects:
        plot_project_survival_curve(project)

if __name__ == '__main__':
    main()