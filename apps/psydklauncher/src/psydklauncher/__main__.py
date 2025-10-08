from psydk_launcher import run_py

if __name__ == "__main__":
    # find the directory of this file
    file_path = __file__
    # list contents of the parent directory
    import os
    parent_dir = os.path.dirname(os.path.dirname(file_path))
    print(f"Parent directory: {parent_dir}")
    for item in os.listdir(parent_dir + "/psydklauncher"):
        print(f"Item: {item}")

    repo_dir = parent_dir + "/psydklauncher/sample_projects"
    data_dir = parent_dir + "/psydklauncher/sample_project/data"
    run_py(repo_dir, data_dir)
