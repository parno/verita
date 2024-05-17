#!/usr/bin/env python

import argparse
import json
import glob
from matplotlib import pyplot as plt
import numpy as np
import os

def plot_survival_curve(times, name, total_solved, errors):
    # Calculate survival curve
    perf = np.array(np.sort(times))
    cdf = np.cumsum(perf)
    plt.plot(cdf, np.arange(0, len(cdf)), label=name, linestyle="solid", color="black")
    plt.title(f"{name} - Solved {total_solved}, with {errors} errors")
    plt.ylim(0)
    plt.xlim(0.1)
    plt.xscale("log")
    plt.xlabel("Time Log Scale (ms)")
    plt.ylabel("Instances Soveld")
    plt.grid()
    os.makedirs("fig/survival", exist_ok=True)
    plt.savefig(f"fig/survival/{name}.pdf")
    plt.close()

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
        self.run_label = json["runner"]["label"]

        # Collect SMT times
        self.fn_smt_times = []
        for item in self.times_ms["smt"]["smt-run-module-times"]:
            for function in item["function-breakdown"]:
                self.fn_smt_times.append(FunctionSmtTime(function))

    def __str__(self):
        return f'{self.name} <{self.refspec}>'

    def plot_survival_curve(self):
        plot_survival_curve([f.time_ms for f in self.fn_smt_times], self.name, self.total_solved, self.errors)


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
        self.label = self.projects[0].run_label

    def get_smt_times(self):
        return [f.time_ms for project in self.projects for f in project.fn_smt_times]

    def __str__(self):
        return f'{self.project} <{self.time_ms}>'

    def plot_survival_curve(self):
        total_solved = sum([project.total_solved for project in self.projects])
        errors = sum([project.errors for project in self.projects])

        plot_survival_curve(self.get_smt_times(), f'{self.label} ({os.path.basename(self.directory)})', total_solved, errors)

    def plot_survival_curves(self):
        self.plot_survival_curve()
        for project in self.projects:
            project.plot_survival_curve()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('dirs', nargs='+', required=True, help='One or more directories of results to analyze')
    args = parser.parse_args()

    runs = [Run(d) for d in args.dirs]

if __name__ == '__main__':
    main()