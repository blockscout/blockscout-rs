import requests
import os
import json

token = os.environ['GITHUB_TOKEN']
host = 'https://api.github.com/repos/sevenzing/test'

headers = {
    'Authorization': f'Bearer {token}',
    'Accept': 'application/vnd.github+json'
}

APP = "autodeploy"
CLIENT = "test-client"

def write_response(filename, url, method, response, override_body=None):

    url = url.replace('https://api.github.com', '').replace('sevenzing', '{owner}').replace('test', '{repo}')
    try:
        if override_body is not None:
            response_data = override_body
        else:
            response_data = response.json()
    except:
        response_data = response.text

    data = {
        'filename': filename,
        'url': url,
        'method': method.upper(),
        'status': response.status_code,
        'response': response_data,
    }
    with open(filename, 'w') as f:
        f.write(json.dumps(data, indent=2))

# get all commits
url = host + '/commits'
r = requests.get(url, headers=headers)
write_response('commits.json', url, 'GET', r)

# get the main branch
url = host + '/commits/main'
r = requests.get(url, headers=headers)
write_response('main.json', url, 'GET', r)
main_sha = r.json()['sha']

# get all workflows
url = host + '/actions/workflows'
r = requests.get(url, headers=headers)
write_response('workflows.json', url, 'GET', r)

# get all workflow runs
workflows = r.json()['workflows']
for workflow in workflows:
    workflow_id = workflow['path'].split('/')[-1]
    url = host + f'/actions/workflows/{workflow_id}/runs'
    r = requests.get(url, headers=headers)
    data = r.json()
    data["workflow_runs"][0]["created_at"] = "2050-01-01T00:00:00Z"
    data["workflow_runs"][0]["updated_at"] = "2050-01-01T00:00:00Z"
    data["workflow_runs"][0]["name"] = f"Deploy {APP} to {CLIENT} env"
    filename = f"runs_{workflow_id.replace('.', '_')}.json"
    write_response(filename, url, 'GET', r, override_body=data)

    # dispatch a workflow
    url = host + f'/actions/workflows/{workflow_id}/dispatches'
    r = requests.post(url, headers=headers, json={"ref":"main", "inputs":{"client": CLIENT, "app": APP}})
    filename = f"dispatch_{workflow_id.replace('.', '_')}.json"
    write_response(filename, url, 'POST', r)

# creating blob
url = host + '/git/blobs'
r = requests.post(url, headers=headers, json={"content":"Content of the blob","encoding":"utf-8"})
write_response('new_blob.json', url, 'POST', r)
blob_sha = r.json()['sha']

# creating tree
data = {
    "base_tree":"8b8551ae8b5d47f848575ac68437a8379bdf8be9",
    "tree":[{"path":"file","mode":"100644","type":"blob","sha":blob_sha}]
}
url = host + '/git/trees'
r = requests.post(url, headers=headers, json=data)
write_response('new_tree.json', url, 'POST', r)
tree_sha = r.json()['sha']

# creating commit
url = host + '/git/commits'
r = requests.post(url, headers=headers, json={"message":"commit message","tree":tree_sha,"parents":[main_sha]})
write_response('new_commit.json', url, 'POST', r)
commit_sha = r.json()['sha']

# updating the main branch
url = host + '/git/refs/heads/main'
r = requests.patch(url, headers=headers, json={"sha": commit_sha})
write_response('update_main.json', url, 'PATCH', r)
