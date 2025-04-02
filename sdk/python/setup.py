from setuptools import setup, find_packages

setup(
    name="microsandbox",
    version="0.1.0",
    packages=find_packages(),
    description="Microsandbox Python SDK",
    long_description=open("README.md").read(),
    long_description_content_type="text/markdown",
    author="Microsandbox Team",
    author_email="team@microsandbox.dev",
    url="https://microsandbox.dev",
    classifiers=[
        "Programming Language :: Python :: 3",
        "License :: OSI Approved :: Apache Software License",
        "Operating System :: OS Independent",
    ],
    python_requires=">=3.6",
    extras_require={
        "dev": [
            "pytest>=6.0.0",
            "black>=22.0.0",
            "isort>=5.0.0",
            "mypy>=0.900",
            "build>=0.8.0",
            "twine>=4.0.0",
        ],
    },
)
